//! Detection pipeline stages and shared constants.

pub mod ascii;
pub mod binary;
pub mod bom;
pub mod confusion;
pub mod escape;
pub mod language;
pub mod magic;
pub mod markup;
pub mod orchestrator;
pub mod postprocess;
pub mod statistical;
pub mod structural;
pub mod utf1632;
pub mod utf8;
pub mod validity;

/// Confidence for deterministic (non-BOM) detection stages.
pub const DETERMINISTIC_CONFIDENCE: f64 = crate::utils::DETERMINISTIC_CONFIDENCE;
