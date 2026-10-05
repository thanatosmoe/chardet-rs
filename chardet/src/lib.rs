//! Universal character encoding detector — Rust port of chardet 7.
#![forbid(unsafe_code)]

pub mod codecs;
pub mod enums;
pub mod models;
pub mod output_names;
pub mod pipeline;
mod registry_data;
pub mod registry;
pub mod result;
pub mod utils;

pub use pipeline::orchestrator::{run_pipeline, PipelineOptions};
pub use result::DetectionResult;

/// Public options for [`detect`] and [`detect_all`].
#[derive(Clone)]
pub struct DetectOptions {
    pub encoding_era: u8,
    pub max_bytes: usize,
    pub prefer_superset: bool,
    /// Return chardet 5.x/6.x compatible encoding names (default `true`).
    pub compat_names: bool,
    /// If given, only these encodings are considered (names or aliases).
    pub include_encodings: Option<Vec<String>>,
    /// If given, these encodings are removed from the candidate set.
    pub exclude_encodings: Option<Vec<String>>,
    /// Encoding returned when no candidate survives (default `cp1252`).
    pub no_match_encoding: String,
    /// Encoding returned for empty input (default `utf-8`).
    pub empty_input_encoding: String,
    /// When `true`, `detect_all` keeps every candidate regardless of confidence.
    pub ignore_threshold: bool,
}

impl Default for DetectOptions {
    fn default() -> Self {
        DetectOptions {
            encoding_era: enums::Era::ALL,
            max_bytes: utils::DEFAULT_MAX_BYTES,
            prefer_superset: false,
            compat_names: true,
            include_encodings: None,
            exclude_encodings: None,
            no_match_encoding: "cp1252".to_string(),
            empty_input_encoding: "utf-8".to_string(),
            ignore_threshold: false,
        }
    }
}

fn canonical_filter(list: &Option<Vec<String>>) -> Result<Option<Vec<String>>, String> {
    match list {
        None => Ok(None),
        Some(items) => {
            let mut out = Vec::with_capacity(items.len());
            for name in items {
                out.push(registry::validate_encoding(name)?.to_string());
            }
            if out.is_empty() {
                return Err("encoding filter must not be empty".to_string());
            }
            Ok(Some(out))
        }
    }
}

fn build_run_options(opts: &DetectOptions) -> Result<PipelineOptions, String> {
    let include = canonical_filter(&opts.include_encodings)?;
    let exclude = canonical_filter(&opts.exclude_encodings)?;
    let no_match = registry::validate_encoding(&opts.no_match_encoding)?.to_string();
    let empty = registry::validate_encoding(&opts.empty_input_encoding)?.to_string();
    Ok(PipelineOptions {
        encoding_era: opts.encoding_era,
        max_bytes: opts.max_bytes,
        include_encodings: include,
        exclude_encodings: exclude,
        no_match_encoding: no_match,
        empty_input_encoding: empty,
        full_ranking: false,
        input_truncated: false,
    })
}

fn apply_output_names(mut result: DetectionResult, data: &[u8], opts: &DetectOptions) -> DetectionResult {
    let window = &data[..data.len().min(opts.max_bytes)];
    if opts.prefer_superset {
        output_names::apply_preferred_superset(&mut result, Some(window));
    }
    if opts.compat_names {
        output_names::apply_compat_names(&mut result);
    }
    result
}

/// Detect the encoding of `data`, applying the public output-name transforms.
///
/// Mirrors `chardet.detect`: prefer-superset remapping (when enabled) followed
/// by chardet 5.x/6.x compatible naming (when enabled).
pub fn detect(data: &[u8]) -> DetectionResult {
    detect_with(data, &DetectOptions::default()).unwrap_or_else(|_| result::none_result())
}

/// Like [`detect`], with explicit options.
pub fn detect_with(data: &[u8], opts: &DetectOptions) -> Result<DetectionResult, String> {
    let run_opts = build_run_options(opts)?;
    let results = run_pipeline(data, &run_opts);
    let result = results
        .into_iter()
        .next()
        .unwrap_or_else(result::none_result);
    Ok(apply_output_names(result, data, opts))
}

/// Return all candidate results sorted by descending confidence.
///
/// Mirrors `chardet.detect_all`: results at or below `MINIMUM_THRESHOLD` are
/// filtered out unless `ignore_threshold` is set (or nothing clears it).
pub fn detect_all(data: &[u8], opts: &DetectOptions) -> Result<Vec<DetectionResult>, String> {
    let mut run_opts = build_run_options(opts)?;
    run_opts.full_ranking = true;
    let results = run_pipeline(data, &run_opts);
    let mut dicts: Vec<DetectionResult> = if opts.ignore_threshold {
        results
    } else {
        let filtered: Vec<DetectionResult> = results
            .iter()
            .filter(|r| r.confidence > utils::MINIMUM_THRESHOLD)
            .cloned()
            .collect();
        if filtered.is_empty() {
            results
        } else {
            filtered
        }
    };
    for d in &mut dicts {
        let out = apply_output_names(d.clone(), data, opts);
        *d = out;
    }
    dicts.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    Ok(dicts)
}
