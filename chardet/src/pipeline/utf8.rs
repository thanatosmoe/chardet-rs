//! Stage 1d: UTF-8 structural validation, mirroring `chardet.pipeline.utf8`.

use crate::result::DetectionResult;

const BASE_CONFIDENCE: f64 = 0.80;
const MAX_CONFIDENCE: f64 = 0.99;
const MB_RATIO_SCALE: f64 = 6.0;

/// Sequence length a UTF-8 lead byte declares (0 for non-lead).
fn expected_seq_len(byte: u8) -> usize {
    match byte {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 0,
    }
}

fn valid_continuation(lead: u8, index: usize, byte: u8) -> bool {
    if index >= 2 {
        return byte & 0xC0 == 0x80;
    }
    match lead {
        0xE0 => (0xA0..=0xBF).contains(&byte),
        0xED => (0x80..=0x9F).contains(&byte),
        0xF0 => (0x90..=0xBF).contains(&byte),
        0xF4 => (0x80..=0x8F).contains(&byte),
        _ => (0x80..=0xBF).contains(&byte),
    }
}

fn count_high(data: &[u8]) -> usize {
    data.iter().filter(|&&b| b >= 0x80).count()
}

/// Validate UTF-8 byte structure, separating validity from evidence.
///
/// Returns `(window_is_valid_utf8, result_or_None)`.
pub fn scan_utf8(data: &[u8]) -> (bool, Option<DetectionResult>) {
    if data.is_empty() {
        return (true, None);
    }
    if data.iter().all(|&b| b < 0x80) {
        return (true, None);
    }

    let n = data.len();
    let total_high = count_high(data);
    let mut pending_len = 0usize;
    let mut error: Option<(usize, usize)> = None; // (lead index, error offset)
    let mut i = 0usize;
    while i < n {
        let b = data[i];
        if b < 0x80 {
            i += 1;
            continue;
        }
        let len = expected_seq_len(b);
        if len == 0 {
            error = Some((i, i + 1));
            break;
        }
        let mut j = 1usize;
        let mut bad = false;
        while j < len {
            if i + j >= n {
                pending_len = n - i;
                i = n;
                break;
            }
            if !valid_continuation(b, j, data[i + j]) {
                error = Some((i, i + j));
                bad = true;
                break;
            }
            j += 1;
        }
        if bad {
            break;
        }
        if i == n {
            break;
        }
        i += len;
    }

    let tail_start = match error {
        None => n - pending_len,
        Some((lead, _end)) => {
            let mut tail_lead: isize = -1;
            for k in n.saturating_sub(3)..n {
                let sl = expected_seq_len(data[k]);
                if sl > 0 && k + sl > n {
                    tail_lead = k as isize;
                    break;
                }
            }
            if tail_lead < 0 || (lead as isize) < tail_lead {
                return (false, None);
            }
            tail_lead as usize
        }
    };

    let mut mb = total_high;
    if tail_start < n {
        mb -= count_high(&data[tail_start..]);
    }
    if mb == 0 {
        return (true, None);
    }
    let mb_ratio = mb as f64 / n as f64;
    let confidence_range = MAX_CONFIDENCE - BASE_CONFIDENCE;
    let confidence = MAX_CONFIDENCE.min(
        BASE_CONFIDENCE + confidence_range * (mb_ratio * MB_RATIO_SCALE).min(1.0),
    );
    (true, Some(DetectionResult::enc("utf-8", confidence)))
}

/// Validate UTF-8, returning a result only when multi-byte evidence exists.
pub fn detect_utf8(data: &[u8]) -> Option<DetectionResult> {
    scan_utf8(data).1
}
