//! Shared pipeline types: `DetectionResult` and `PipelineContext`.

use std::collections::HashMap;

/// A single encoding detection result.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectionResult {
    pub encoding: Option<String>,
    pub confidence: f64,
    pub language: Option<String>,
    pub mime_type: Option<String>,
}

impl DetectionResult {
    pub fn new(encoding: Option<String>, confidence: f64, language: Option<String>) -> Self {
        DetectionResult {
            encoding,
            confidence,
            language,
            mime_type: None,
        }
    }

    pub fn with_mime(
        encoding: Option<String>,
        confidence: f64,
        language: Option<String>,
        mime_type: Option<String>,
    ) -> Self {
        DetectionResult {
            encoding,
            confidence,
            language,
            mime_type,
        }
    }

    pub fn enc(s: &str, confidence: f64) -> Self {
        DetectionResult {
            encoding: Some(s.to_string()),
            confidence,
            language: None,
            mime_type: None,
        }
    }
}

/// Sentinel result for "no detection".
pub fn none_result() -> DetectionResult {
    DetectionResult::new(None, 0.0, None)
}

/// Per-run mutable state for a single pipeline invocation.
#[derive(Debug, Default)]
pub struct PipelineContext {
    /// Cached per-encoding structural analysis: (pair_ratio, mb_bytes, lead_diversity).
    pub analysis_cache: HashMap<&'static str, (f64, usize, usize)>,
    pub non_ascii_count: Option<usize>,
    pub mb_scores: HashMap<String, f64>,
    pub mb_coverage: HashMap<String, f64>,
}
