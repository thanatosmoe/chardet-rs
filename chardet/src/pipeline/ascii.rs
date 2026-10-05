//! Stage 1c: Pure ASCII detection (with null-separator tolerance).

use crate::result::DetectionResult;
use crate::utils::{count_deleted, ASCII_TEXT_BYTES};

const MAX_NULL_FRACTION: f64 = 0.05;

/// Return an ASCII result if all bytes are printable ASCII plus whitespace.
pub fn detect_ascii(data: &[u8]) -> Option<DetectionResult> {
    if data.is_empty() {
        return None;
    }
    if !data.iter().all(|&b| b < 0x80) {
        return None;
    }
    let disallowed = data.len() - count_deleted(data, ASCII_TEXT_BYTES);
    if disallowed == 0 {
        return Some(DetectionResult::enc("ascii", 1.0));
    }
    let null_count = data.iter().filter(|&&b| b == 0).count();
    if disallowed != null_count {
        return None;
    }
    let null_fraction = null_count as f64 / data.len() as f64;
    if null_fraction <= MAX_NULL_FRACTION {
        return Some(DetectionResult::enc("ascii", 0.99));
    }
    None
}
