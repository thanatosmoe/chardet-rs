//! Stage 0: Binary content detection.

use crate::utils::{count_deleted, DEFAULT_MAX_BYTES};

const BINARY_THRESHOLD: f64 = 0.01;
const EBCDIC_MIN_HIGH_FRACTION: f64 = 0.25;
const EBCDIC_MIN_SPACE_FRACTION: f64 = 0.02;
const EBCDIC_SPACE: u8 = 0x40;
const EBCDIC_HT: u8 = 0x05;
const EBCDIC_NL: u8 = 0x15;

fn binary_delete() -> Vec<u8> {
    let mut v: Vec<u8> = (0..0x09).collect();
    v.extend(0x0E..0x20);
    v
}

/// Return `true` if `data` appears to be binary (not text) content.
pub fn is_binary(data: &[u8], max_bytes: usize) -> bool {
    let data = &data[..data.len().min(max_bytes)];
    if data.is_empty() {
        return false;
    }
    let del = binary_delete();
    let binary_count = count_deleted(data, &del);
    if binary_count as f64 / data.len() as f64 <= BINARY_THRESHOLD {
        return false;
    }
    let ht_count = data.iter().filter(|&&b| b == EBCDIC_HT).count();
    let nl_count = data.iter().filter(|&&b| b == EBCDIC_NL).count();
    let hard_count = binary_count - ht_count - nl_count;
    if hard_count as f64 / data.len() as f64 > BINARY_THRESHOLD {
        return true;
    }
    let space_count = data.iter().filter(|&&b| b == EBCDIC_SPACE).count() + ht_count;
    let non_space = data.len() - space_count;
    if non_space == 0 {
        return false;
    }
    let high: Vec<u8> = (0x80..=0xFF).collect();
    let high_count = count_deleted(data, &high);
    if high_count as f64 / (non_space as f64) < EBCDIC_MIN_HIGH_FRACTION {
        return true;
    }
    (space_count as f64 / data.len() as f64) < EBCDIC_MIN_SPACE_FRACTION
}

pub fn is_binary_default(data: &[u8]) -> bool {
    is_binary(data, DEFAULT_MAX_BYTES)
}
