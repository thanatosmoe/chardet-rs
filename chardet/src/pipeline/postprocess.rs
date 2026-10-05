//! Stage 13: post-processing rank corrections.

use crate::codecs::{dangling_tail_with_ascii_prefix, decodes_completely, decodes_without_error};
use crate::models::{model_data, ART_LANGUAGE, RARE_LANGUAGES};
use crate::output_names::COMPAT_NAMES;
use crate::pipeline::confusion::{
    arbitrate_distinguishing_bytes, confusion_pair_winner, differing_high_bytes,
    resolve_confusion_groups, CONFUSION_BAND, CONFUSION_FLOOR_RATIO, STRICT_TIER_MAX_CONF,
};
use crate::registry::lookup;
use crate::result::DetectionResult;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};

static COMMON_LATIN: &[&str] = &["iso8859-1", "iso8859-15", "cp1252"];

fn set(bytes: &[u8]) -> HashSet<u8> {
    bytes.iter().copied().collect()
}

static DEMOTION_CANDIDATES: Lazy<HashMap<&'static str, HashSet<u8>>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(
        "iso8859-10",
        set(&[
            0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAE, 0xAF, 0xB1,
            0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0,
            0xC7, 0xC8, 0xCA, 0xCC, 0xD1, 0xD2, 0xD7, 0xD9, 0xE0, 0xE7, 0xE8, 0xEA, 0xEC, 0xF1,
            0xF2, 0xF7, 0xF9, 0xFF,
        ]),
    );
    m.insert(
        "iso8859-14",
        set(&[
            0xA1, 0xA2, 0xA4, 0xA5, 0xA6, 0xA8, 0xAA, 0xAB, 0xAC, 0xAF, 0xB0, 0xB1, 0xB2, 0xB3,
            0xB4, 0xB5, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xD0, 0xD7, 0xDE,
            0xF0, 0xF7, 0xFE,
        ]),
    );
    m.insert("cp1254", set(&[0xD0, 0xDD, 0xDE, 0xF0, 0xFD, 0xFE]));
    m.insert(
        "hp-roman8",
        set(&[
            0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD,
            0xCE, 0xCF, 0xD1, 0xD4, 0xD5, 0xD6, 0xD9, 0xDD, 0xDE,
        ]),
    );
    m
});

static KOI8_T_DISTINGUISHING: Lazy<HashSet<u8>> =
    Lazy::new(|| set(&[0x80, 0x81, 0x83, 0x8A, 0x8C, 0x8D, 0x8E, 0x90, 0xA1, 0xA2, 0xA5, 0xB5]));

const DEAD_HEAT_EPSILON: f64 = 1e-4;
const CR_MAC_BAND: f64 = CONFUSION_BAND;
const CR_MAC_MIN_LINES: usize = 3;
const LEGACY_MAC_ERA: u8 = 4;
const EVIDENCE_SCAN_MAX_BYTES: usize = 16384;

fn era_rank(encoding: &str) -> u32 {
    match lookup(encoding) {
        Some(info) => {
            let era = info.era;
            (era & era.wrapping_neg()) as u32
        }
        None => 1 << 30,
    }
}

fn has_byte(data: &[u8], set: &HashSet<u8>) -> bool {
    data.iter().any(|b| set.contains(b))
}

fn should_demote(data: &[u8], top: &DetectionResult, target: &DetectionResult) -> bool {
    let encoding = top.encoding.clone().unwrap_or_default();
    let set = match DEMOTION_CANDIDATES.get(encoding.as_str()) {
        Some(s) => s,
        None => return false,
    };
    if !has_byte(data, set) {
        return true;
    }
    if top.confidence - target.confidence > CONFUSION_BAND {
        return false;
    }
    let lang_a = top
        .language
        .as_ref()
        .map(|l| HashSet::from([l.clone()]));
    let lang_b = target
        .language
        .as_ref()
        .map(|l| HashSet::from([l.clone()]));
    let winner = arbitrate_distinguishing_bytes(
        data,
        &encoding,
        target.encoding.as_deref().unwrap_or(""),
        set,
        lang_a.as_ref(),
        lang_b.as_ref(),
    );
    winner.as_deref() != Some(encoding.as_str())
}

fn swap_target_idx(results: &[DetectionResult]) -> Option<usize> {
    let mut lead_conf = f64::MIN;
    for r in &results[1..] {
        if let Some(e) = r.encoding.as_deref() {
            if COMMON_LATIN.contains(&e) {
                lead_conf = lead_conf.max(r.confidence);
            }
        }
    }
    if lead_conf == f64::MIN {
        return None;
    }
    let mut best: Option<usize> = None;
    let mut best_rank = u32::MAX;
    for (i, r) in results.iter().enumerate().skip(1) {
        let e = match r.encoding.as_deref() {
            Some(e) if COMMON_LATIN.contains(&e) => e,
            _ => continue,
        };
        if lead_conf - r.confidence > DEAD_HEAT_EPSILON {
            continue;
        }
        let rank = era_rank(e);
        if best.is_none() || rank < best_rank {
            best = Some(i);
            best_rank = rank;
        }
    }
    best
}

fn demote_niche_latin(data: &[u8], results: Vec<DetectionResult>) -> Vec<DetectionResult> {
    if results.len() < 2 {
        return results;
    }
    let top_enc = match results[0].encoding.as_deref() {
        Some(e) if DEMOTION_CANDIDATES.contains_key(e) => e.to_string(),
        _ => return results,
    };
    if !results[1..].iter().any(|r| {
        r.encoding
            .as_deref()
            .map(|e| COMMON_LATIN.contains(&e))
            .unwrap_or(false)
    }) {
        return results;
    }
    let target_idx = match swap_target_idx(&results) {
        Some(i) => i,
        None => return results,
    };
    let target = results[target_idx].clone();
    if !should_demote(data, &results[0], &target) {
        return results;
    }
    let top_conf = results[0].confidence;
    let promoted = DetectionResult::with_mime(
        target.encoding.clone(),
        top_conf,
        target.language.clone(),
        target.mime_type.clone(),
    );
    let others: Vec<DetectionResult> = results
        .iter()
        .enumerate()
        .filter(|(i, x)| {
            x.encoding.as_deref() != Some(top_enc.as_str()) && *i != target_idx
        })
        .map(|(_, x)| x.clone())
        .collect();
    let tail_conf = others.last().map(|x| x.confidence).unwrap_or(top_conf);
    let demoted: Vec<DetectionResult> = results
        .iter()
        .filter(|x| x.encoding.as_deref() == Some(top_enc.as_str()))
        .map(|x| {
            DetectionResult::with_mime(
                x.encoding.clone(),
                x.confidence.min(tail_conf),
                x.language.clone(),
                x.mime_type.clone(),
            )
        })
        .collect();
    let mut out = vec![promoted];
    out.extend(others);
    out.extend(demoted);
    out
}

fn promote_to_top(results: &[DetectionResult], i: usize) -> Vec<DetectionResult> {
    let r = &results[i];
    let promoted = DetectionResult::with_mime(
        r.encoding.clone(),
        results[0].confidence,
        r.language.clone(),
        r.mime_type.clone(),
    );
    let mut out = vec![promoted];
    for (j, x) in results.iter().enumerate() {
        if j != i {
            out.push(x.clone());
        }
    }
    out
}

fn promote_koi8t(data: &[u8], results: Vec<DetectionResult>) -> Vec<DetectionResult> {
    if results.is_empty() || results[0].encoding.as_deref() != Some("koi8-r") {
        return results;
    }
    let idx = results
        .iter()
        .position(|r| r.encoding.as_deref() == Some("koi8-t"));
    match idx {
        Some(i) if has_byte(data, &KOI8_T_DISTINGUISHING) => promote_to_top(&results, i),
        _ => results,
    }
}

fn has_high_byte_evidence(data: &[u8], encoding: &str, language: Option<&str>) -> bool {
    let md = model_data();
    let variants = match md.variants(encoding) {
        Some(v) if !v.is_empty() => v,
        _ => return false,
    };
    let window = &data[..data.len().min(EVIDENCE_SCAN_MAX_BYTES)];
    let mut seen: HashSet<u32> = HashSet::new();
    if window.len() >= 2 {
        let mut prev = window[0];
        for &b in &window[1..] {
            if prev >= 0x80 || b >= 0x80 {
                seen.insert(((prev as u32) << 8) | b as u32);
            }
            prev = b;
        }
    }
    if seen.is_empty() {
        return false;
    }
    for &i in variants {
        let v = md.variant(i);
        if let (Some(lang), Some(ref vlang)) = (language, &v.lang) {
            if vlang != lang {
                continue;
            }
        }
        for &idx in &seen {
            if v.table[idx as usize] != 0 {
                return true;
            }
        }
    }
    false
}

fn prefer_prevalent_on_dead_heat(
    data: &[u8],
    results: Vec<DetectionResult>,
) -> Vec<DetectionResult> {
    if results.is_empty() || results[0].encoding.is_none() || results.len() < 2 {
        return results;
    }
    let top = results[0].clone();
    let mut best_idx = 0usize;
    let mut best_rank = era_rank(top.encoding.as_deref().unwrap());
    for (i, r) in results.iter().enumerate().skip(1) {
        if r.encoding.is_none() {
            continue;
        }
        if top.confidence - r.confidence > DEAD_HEAT_EPSILON {
            break;
        }
        let rank = era_rank(r.encoding.as_deref().unwrap());
        if rank < best_rank {
            best_rank = rank;
            best_idx = i;
        }
    }
    if best_idx == 0 {
        return results;
    }
    let top_enc = top.encoding.clone().unwrap();
    if !has_high_byte_evidence(data, &top_enc, top.language.as_deref()) {
        return promote_to_top(&results, best_idx);
    }
    let rival = results[best_idx].encoding.clone().unwrap();
    let langs: HashSet<String> = [top.language.clone(), results[best_idx].language.clone()]
        .into_iter()
        .flatten()
        .collect();
    let comparable = crate::pipeline::confusion::comparable_languages_public(&top_enc, &rival, &langs);
    let diff = differing_high_bytes(&top_enc, &rival);
    let winner = arbitrate_distinguishing_bytes(
        data,
        &top_enc,
        &rival,
        &diff,
        comparable.as_ref(),
        comparable.as_ref(),
    );
    if winner.as_deref() == Some(rival.as_str()) {
        return promote_to_top(&results, best_idx);
    }
    results
}

fn promote_superset_on_dead_heat(
    data: &[u8],
    results: Vec<DetectionResult>,
) -> Vec<DetectionResult> {
    if results.is_empty() || results[0].encoding.is_none() || results.len() < 2 {
        return results;
    }
    let top = &results[0];
    let superset = match top.encoding.as_deref() {
        Some("shift_jis") | Some("shift_jis_2004") => "cp932",
        Some("euc_kr") => "cp949",
        _ => return results,
    };
    for (i, r) in results.iter().enumerate().skip(1) {
        if top.confidence - r.confidence > DEAD_HEAT_EPSILON {
            break;
        }
        if r.encoding.as_deref() == Some(superset) && decodes_without_error(data, superset) {
            return promote_to_top(&results, i);
        }
    }
    results
}

const RARE_ARBITRATION_MARGIN: f64 = 0.02;
const RARE_ARBITRATION_MAX_CONFIDENCE: f64 = 0.15;

fn arbitrate_rare_language(results: Vec<DetectionResult>) -> Vec<DetectionResult> {
    if results.is_empty() {
        return results;
    }
    let top = &results[0];
    let top_lang = match top.language.as_deref() {
        Some(l) => l,
        None => return results,
    };
    if top.encoding.is_none()
        || !RARE_LANGUAGES.contains(&top_lang)
        || top.confidence >= RARE_ARBITRATION_MAX_CONFIDENCE
        || results.len() < 2
    {
        return results;
    }
    for (i, r) in results.iter().enumerate().skip(1) {
        if top.confidence - r.confidence > RARE_ARBITRATION_MARGIN {
            break;
        }
        if r.encoding.is_none() || r.language.is_none() {
            continue;
        }
        let lang = r.language.as_deref().unwrap();
        if !RARE_LANGUAGES.contains(&lang) {
            return promote_to_top(&results, i);
        }
    }
    results
}

fn promote_mac_on_cr_line_endings(
    data: &[u8],
    results: Vec<DetectionResult>,
) -> Vec<DetectionResult> {
    if results.is_empty() || results[0].encoding.is_none() || results.len() < 2 {
        return results;
    }
    let top = results[0].clone();
    if era_rank(top.encoding.as_deref().unwrap()) == LEGACY_MAC_ERA as u32 {
        return results;
    }
    if top.language.as_deref() == Some(ART_LANGUAGE) {
        return results;
    }
    if data.contains(&b'\n') || data.iter().filter(|&&b| b == b'\r').count() < CR_MAC_MIN_LINES {
        return results;
    }
    for (i, r) in results.iter().enumerate().skip(1) {
        if top.confidence - r.confidence > CR_MAC_BAND {
            break;
        }
        if let Some(enc) = r.encoding.as_deref() {
            if era_rank(enc) == LEGACY_MAC_ERA as u32 {
                let langs: HashSet<String> =
                    [top.language.clone(), r.language.clone()].into_iter().flatten().collect();
                if confusion_pair_winner(data, top.encoding.as_deref().unwrap(), enc, &langs)
                    .as_deref()
                    == Some(top.encoding.as_deref().unwrap())
                {
                    break;
                }
                return promote_to_top(&results, i);
            }
        }
    }
    results
}

fn decodes_under_public_names(data: &[u8], encoding: &str) -> bool {
    if !decodes_completely(data, encoding) {
        return false;
    }
    let display = COMPAT_NAMES.get(encoding);
    match display {
        None => true,
        Some(d) => decodes_completely(data, d),
    }
}

fn prefer_decodable_on_tie(
    data: &[u8],
    results: Vec<DetectionResult>,
    input_truncated: bool,
) -> Vec<DetectionResult> {
    if input_truncated || results.is_empty() || results[0].encoding.is_none() || results.len() < 2
    {
        return results;
    }
    let tail = &data[data.len().saturating_sub(4)..];
    if !tail.iter().any(|&b| b >= 0x80) {
        return results;
    }
    let top_enc = results[0].encoding.clone().unwrap();
    if !dangling_tail_with_ascii_prefix(data, &top_enc) {
        return results;
    }
    for (i, r) in results.iter().enumerate().skip(1) {
        let enc = match r.encoding.as_deref() {
            Some(e) => e,
            None => continue,
        };
        if decodes_under_public_names(data, enc) {
            return promote_to_top(&results, i);
        }
    }
    results
}

/// Apply rank corrections to the statistically scored results.
pub fn postprocess_results(
    data: &[u8],
    results: Vec<DetectionResult>,
    input_truncated: bool,
) -> Vec<DetectionResult> {
    let results = promote_superset_on_dead_heat(data, results);
    let results = prefer_prevalent_on_dead_heat(data, results);
    let results = arbitrate_rare_language(results);
    let results = resolve_confusion_groups(data, results);
    let results = demote_niche_latin(data, results);
    let results = promote_koi8t(data, results);
    let results = promote_mac_on_cr_line_endings(data, results);
    prefer_decodable_on_tie(data, results, input_truncated)
}

/// Public wrapper for the pruning floor (retained for API completeness).
pub fn scoring_floor(top1: f64, top2: f64) -> f64 {
    let correction_reach = RARE_ARBITRATION_MARGIN + 2.0 * CONFUSION_BAND;
    let mut floor = top2 - correction_reach;
    if top1 < STRICT_TIER_MAX_CONF {
        floor = floor.min(top1 * CONFUSION_FLOOR_RATIO);
    }
    floor
}
