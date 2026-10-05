//! Pipeline orchestrator — runs all detection stages in sequence.

use crate::codecs::decodes_without_error;
use crate::enums::Era;
use crate::models::ART_LANGUAGE;
use crate::pipeline::{
    ascii::detect_ascii,
    binary::is_binary,
    bom::detect_bom,
    escape::detect_escape_encoding,
    language::fill_languages,
    magic::detect_magic,
    markup::{detect_markup_charset, promote_markup_superset},
    postprocess::postprocess_results,
    statistical::score_candidates,
    structural::{
        compute_lead_byte_diversity, compute_multibyte_byte_coverage, compute_structural_score,
    },
    utf1632::detect_utf1632_patterns,
    utf8::scan_utf8,
    validity::filter_by_validity,
};
use crate::registry::{get_candidates, EncodingInfo};
use crate::result::{none_result, DetectionResult, PipelineContext};
use crate::utils::{DETERMINISTIC_CONFIDENCE, EVIDENCE_CAP_BYTES};

const STRUCTURAL_CONFIDENCE_THRESHOLD: f64 = 0.85;
const STAT_SCORE_MAX_BYTES: usize = 16384;
const CJK_MIN_MB_RATIO: f64 = 0.05;
const CJK_MIN_NON_ASCII: usize = 2;
const CJK_MIN_BYTE_COVERAGE: f64 = 0.35;
const CJK_MIN_LEAD_DIVERSITY: usize = 4;
const CJK_DIVERSITY_MIN_NON_ASCII: usize = 16;

/// Options controlling a pipeline run.
pub struct PipelineOptions {
    pub encoding_era: u8,
    pub max_bytes: usize,
    pub include_encodings: Option<Vec<String>>,
    pub exclude_encodings: Option<Vec<String>>,
    pub no_match_encoding: String,
    pub empty_input_encoding: String,
    pub full_ranking: bool,
    pub input_truncated: bool,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        PipelineOptions {
            encoding_era: Era::ALL,
            max_bytes: crate::utils::DEFAULT_MAX_BYTES,
            include_encodings: None,
            exclude_encodings: None,
            no_match_encoding: "cp1252".to_string(),
            empty_input_encoding: "utf-8".to_string(),
            full_ranking: false,
            input_truncated: false,
        }
    }
}

fn make_fallback_or_none(
    encoding: &str,
    allowed: &[&str],
) -> Vec<DetectionResult> {
    if !allowed.contains(&encoding) {
        return vec![none_result()];
    }
    vec![DetectionResult::new(Some(encoding.to_string()), 0.10, None)]
}

fn hold_validity_past_cap(
    data: &[u8],
    evidence: &[u8],
    results: Vec<DetectionResult>,
    allowed: &[&str],
    no_match_encoding: &str,
) -> Vec<DetectionResult> {
    if data.len() <= evidence.len() {
        return results;
    }
    for (i, r) in results.iter().enumerate() {
        if let Some(enc) = &r.encoding {
            if decodes_without_error(data, enc) {
                return if i == 0 {
                    results
                } else {
                    results[i..].to_vec()
                };
            }
        }
    }
    make_fallback_or_none(no_match_encoding, allowed)
}

fn gate_cjk_candidates<'a>(
    data: &[u8],
    valid_candidates: &[&'a EncodingInfo],
    ctx: &mut PipelineContext,
) -> Vec<&'a EncodingInfo> {
    let mut gated: Vec<&'a EncodingInfo> = Vec::new();
    for enc in valid_candidates {
        if enc.is_multibyte {
            let mb_score = compute_structural_score(data, enc, ctx);
            ctx.mb_scores.insert(enc.name.to_string(), mb_score);
            if mb_score < CJK_MIN_MB_RATIO {
                continue;
            }
            if ctx.non_ascii_count.is_none() {
                ctx.non_ascii_count = Some(data.iter().filter(|&&b| b >= 0x80).count());
            }
            let non_ascii = ctx.non_ascii_count.unwrap();
            if non_ascii < CJK_MIN_NON_ASCII {
                continue;
            }
            let byte_coverage =
                compute_multibyte_byte_coverage(data, enc, ctx, Some(non_ascii));
            ctx.mb_coverage.insert(enc.name.to_string(), byte_coverage);
            if byte_coverage < CJK_MIN_BYTE_COVERAGE {
                continue;
            }
            if non_ascii >= CJK_DIVERSITY_MIN_NON_ASCII {
                let lead_diversity = compute_lead_byte_diversity(data, enc, ctx);
                if lead_diversity < CJK_MIN_LEAD_DIVERSITY {
                    continue;
                }
            }
        }
        gated.push(*enc);
    }
    gated
}

fn score_structural_candidates(
    data: &[u8],
    structural_scores: &[(String, f64)],
    valid_candidates: &[&EncodingInfo],
    ctx: &PipelineContext,
    full_ranking: bool,
) -> Vec<DetectionResult> {
    let mut enc_lookup: std::collections::HashMap<&str, &EncodingInfo> =
        std::collections::HashMap::new();
    for e in valid_candidates {
        if e.is_multibyte {
            enc_lookup.insert(e.name, *e);
        }
    }
    let mut combined: Vec<&EncodingInfo> = Vec::new();
    for (name, _s) in structural_scores {
        if let Some(e) = enc_lookup.get(name.as_str()) {
            combined.push(*e);
        }
    }
    for e in valid_candidates {
        if !e.is_multibyte {
            combined.push(*e);
        }
    }
    let results = score_candidates(data, &combined, full_ranking);
    let mut boosted: Vec<DetectionResult> = Vec::with_capacity(results.len());
    for r in results {
        let coverage = r
            .encoding
            .as_ref()
            .and_then(|e| ctx.mb_coverage.get(e))
            .copied()
            .unwrap_or(0.0);
        if coverage >= 0.95 {
            boosted.push(DetectionResult::with_mime(
                r.encoding.clone(),
                r.confidence * (1.0 + coverage),
                r.language.clone(),
                r.mime_type.clone(),
            ));
        } else {
            boosted.push(r);
        }
    }
    boosted.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    boosted
}

fn with_default_mime(mut result: DetectionResult) -> DetectionResult {
    if result.mime_type.is_some() {
        return result;
    }
    result.mime_type = Some(
        if result.encoding.is_some() {
            "text/plain"
        } else {
            "application/octet-stream"
        }
        .to_string(),
    );
    result
}

fn run_pipeline_core(data: &[u8], opts: &PipelineOptions) -> Vec<DetectionResult> {
    let mut ctx = PipelineContext::default();
    let input_truncated = opts.input_truncated || data.len() > opts.max_bytes;
    let data = &data[..data.len().min(opts.max_bytes)];

    let candidates = get_candidates(
        opts.encoding_era,
        opts.include_encodings.as_deref(),
        opts.exclude_encodings.as_deref(),
    );
    let allowed: Vec<&str> = candidates.iter().map(|e| e.name).collect();

    if data.is_empty() {
        return make_fallback_or_none(&opts.empty_input_encoding, &allowed);
    }

    if let Some(r) = detect_bom(data) {
        if let Some(enc) = &r.encoding {
            if allowed.contains(&enc.as_str()) {
                return vec![r];
            }
        }
    }

    if let Some(r) = detect_utf1632_patterns(data) {
        if let Some(enc) = &r.encoding {
            if allowed.contains(&enc.as_str()) {
                return vec![r];
            }
        }
    }

    if let Some(r) = detect_escape_encoding(data) {
        if let Some(enc) = &r.encoding {
            if allowed.contains(&enc.as_str()) {
                return vec![r];
            }
        }
    }

    if let Some(r) = detect_magic(data) {
        return vec![r];
    }

    let (utf8_valid, utf8_precheck) = scan_utf8(data);
    let ascii_precheck = detect_ascii(data);

    if utf8_precheck.is_none()
        && ascii_precheck.is_none()
        && is_binary(data, opts.max_bytes)
    {
        return vec![DetectionResult::with_mime(
            None,
            DETERMINISTIC_CONFIDENCE,
            None,
            Some("application/octet-stream".to_string()),
        )];
    }

    if let Some(markup_result) = detect_markup_charset(data) {
        if let Some(menc) = &markup_result.encoding {
            if allowed.contains(&menc.as_str()) {
                if let Some(pre) = &utf8_precheck {
                    if let Some(penc) = &pre.encoding {
                        if penc != menc && allowed.contains(&penc.as_str()) {
                            return vec![DetectionResult::with_mime(
                                pre.encoding.clone(),
                                pre.confidence,
                                pre.language.clone(),
                                markup_result.mime_type.clone(),
                            )];
                        }
                    }
                }
                let promoted =
                    promote_markup_superset(data, markup_result, &allowed);
                return vec![promoted];
            }
        }
    }

    if let Some(r) = &ascii_precheck {
        if let Some(enc) = &r.encoding {
            if allowed.contains(&enc.as_str()) {
                return vec![r.clone()];
            }
        }
    }

    if let Some(r) = &utf8_precheck {
        if let Some(enc) = &r.encoding {
            if allowed.contains(&enc.as_str()) {
                return vec![r.clone()];
            }
        }
    }

    let evidence = &data[..data.len().min(EVIDENCE_CAP_BYTES)];
    let evidence_truncated = input_truncated || data.len() > evidence.len();

    let mut valid_candidates = filter_by_validity(evidence, &candidates);
    if !utf8_valid && data.len() > evidence.len() {
        valid_candidates.retain(|e| e.name != "utf-8");
    }
    if valid_candidates.is_empty() {
        return make_fallback_or_none(&opts.no_match_encoding, &allowed);
    }

    let valid_candidates = gate_cjk_candidates(evidence, &valid_candidates, &mut ctx);
    if valid_candidates.is_empty() {
        return make_fallback_or_none(&opts.no_match_encoding, &allowed);
    }

    let mut structural_scores: Vec<(String, f64)> = Vec::new();
    for enc in &valid_candidates {
        if enc.is_multibyte {
            let score = ctx
                .mb_scores
                .get(enc.name)
                .copied()
                .unwrap_or_else(|| compute_structural_score(evidence, enc, &mut ctx));
            if score > 0.0 {
                structural_scores.push((enc.name.to_string(), score));
            }
        }
    }

    if !structural_scores.is_empty() {
        structural_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        if structural_scores[0].1 >= STRUCTURAL_CONFIDENCE_THRESHOLD {
            let stat_data = &evidence[..evidence.len().min(STAT_SCORE_MAX_BYTES)];
            let results = score_structural_candidates(
                stat_data,
                &structural_scores,
                &valid_candidates,
                &ctx,
                opts.full_ranking,
            );
            if !results.is_empty() {
                let results = postprocess_results(evidence, results, evidence_truncated);
                return hold_validity_past_cap(
                    data,
                    evidence,
                    results,
                    &allowed,
                    &opts.no_match_encoding,
                );
            }
        }
    }

    let stat_data = &evidence[..evidence.len().min(STAT_SCORE_MAX_BYTES)];
    let results = score_candidates(stat_data, &valid_candidates, opts.full_ranking);
    if results.is_empty() {
        return make_fallback_or_none(&opts.no_match_encoding, &allowed);
    }
    let results = postprocess_results(evidence, results, evidence_truncated);
    hold_validity_past_cap(data, evidence, results, &allowed, &opts.no_match_encoding)
}

/// Run the full detection pipeline, returning results sorted by confidence.
pub fn run_pipeline(data: &[u8], opts: &PipelineOptions) -> Vec<DetectionResult> {
    let mut results = run_pipeline_core(data, opts);
    let window = &data[..data.len().min(opts.max_bytes)];
    results = fill_languages(window, results);
    for r in &mut results {
        if r.language.as_deref() == Some(ART_LANGUAGE) {
            r.language = None;
        }
    }
    let mut results: Vec<DetectionResult> = results.into_iter().map(with_default_mime).collect();
    for r in &mut results {
        if r.confidence > 1.0 {
            r.confidence = 1.0;
        }
    }
    results
}

/// Convenience: run the pipeline with default options.
pub fn detect(data: &[u8]) -> Vec<DetectionResult> {
    run_pipeline(data, &PipelineOptions::default())
}
