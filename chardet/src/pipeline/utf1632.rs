//! Stage 1a+: UTF-16/UTF-32 detection for data without BOM.

use crate::codecs::{category_of_cp, is_printable_cp};
use crate::result::DetectionResult;
use crate::utils::ASCII_TEXT_BYTES;

const SAMPLE_SIZE: usize = 4096;
const MIN_BYTES_UTF32: usize = 16;
const MIN_BYTES_UTF16: usize = 10;
const UTF16_MIN_NULL_FRACTION: f64 = 0.03;
const MIN_TEXT_QUALITY: f64 = 0.5;
const QUALITY_TIE_MARGIN: f64 = 0.05;
const MIN_PRINTABLE_FRACTION: f64 = 0.7;
const NULL_SEPARATOR_MAX_FRACTION: f64 = 0.15;

fn decode_utf16(data: &[u8], le: bool) -> Option<String> {
    if data.len() % 2 != 0 {
        return None;
    }
    let mut units = Vec::with_capacity(data.len() / 2);
    let mut i = 0;
    while i + 1 < data.len() {
        let u = if le {
            u16::from_le_bytes([data[i], data[i + 1]])
        } else {
            u16::from_be_bytes([data[i], data[i + 1]])
        };
        units.push(u);
        i += 2;
    }
    String::from_utf16(&units).ok()
}

fn decode_utf32(data: &[u8], le: bool) -> Option<String> {
    if data.len() % 4 != 0 {
        return None;
    }
    let mut s = String::with_capacity(data.len() / 4);
    let mut i = 0;
    while i + 3 < data.len() {
        let u = if le {
            u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        } else {
            u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        };
        s.push(char::from_u32(u)?);
        i += 4;
    }
    Some(s)
}

fn is_null_separator_pattern(data: &[u8], null_frac: f64) -> bool {
    if null_frac >= NULL_SEPARATOR_MAX_FRACTION {
        return false;
    }
    let mut allowed = [false; 256];
    allowed[0] = true;
    for &b in ASCII_TEXT_BYTES {
        allowed[b as usize] = true;
    }
    data.iter().all(|&b| allowed[b as usize])
}

fn looks_like_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let sample: Vec<char> = text.chars().take(500).collect();
    let printable = sample
        .iter()
        .filter(|&&c| is_printable_cp(c as u32) || c == '\n' || c == '\r' || c == '\t')
        .count();
    printable as f64 / sample.len() as f64 > MIN_PRINTABLE_FRACTION
}

fn text_quality(text: &str, limit: usize) -> f64 {
    let sample: Vec<char> = text.chars().take(limit).collect();
    let n = sample.len();
    if n == 0 {
        return -1.0;
    }
    let mut letters = 0usize;
    let mut marks = 0usize;
    let mut spaces = 0usize;
    let mut controls = 0usize;
    let mut ascii_letters = 0usize;
    for &c in &sample {
        let cat = category_of_cp(c as u32);
        let first = cat.as_bytes()[0] as char;
        if first == 'L' {
            letters += 1;
            if (c as u32) < 128 {
                ascii_letters += 1;
            }
        } else if first == 'M' {
            marks += 1;
        } else if cat == "Zs" || c == '\n' || c == '\r' || c == '\t' {
            spaces += 1;
        } else if first == 'C' {
            controls += 1;
        }
    }
    if controls as f64 / n as f64 > 0.1 {
        return -1.0;
    }
    if marks as f64 / n as f64 > 0.2 {
        return -1.0;
    }
    let mut score = letters as f64 / n as f64;
    score += (ascii_letters as f64 / n as f64) * 0.5;
    if n > 20 && spaces > 0 {
        score += 0.1;
    }
    score
}

fn check_utf32(data: &[u8]) -> Option<DetectionResult> {
    let trimmed_len = data.len() - (data.len() % 4);
    if trimmed_len < MIN_BYTES_UTF32 {
        return None;
    }
    let data = &data[..trimmed_len];
    let num_units = trimmed_len / 4;

    let be_first_null = (0..data.len()).step_by(4).filter(|&i| data[i] == 0).count();
    let be_second_null = (0..data.len())
        .step_by(4)
        .filter(|&i| data[i + 1] == 0)
        .count();
    if be_first_null == num_units && be_second_null as f64 / num_units as f64 > 0.5 {
        if let Some(text) = decode_utf32(data, false) {
            if looks_like_text(&text) {
                return Some(DetectionResult::enc(
                    "utf-32-be",
                    crate::utils::DETERMINISTIC_CONFIDENCE,
                ));
            }
        }
    }

    let le_last_null = (3..data.len()).step_by(4).filter(|&i| data[i] == 0).count();
    let le_third_null = (2..data.len()).step_by(4).filter(|&i| data[i] == 0).count();
    if le_last_null == num_units && le_third_null as f64 / num_units as f64 > 0.5 {
        if let Some(text) = decode_utf32(data, true) {
            if looks_like_text(&text) {
                return Some(DetectionResult::enc(
                    "utf-32-le",
                    crate::utils::DETERMINISTIC_CONFIDENCE,
                ));
            }
        }
    }
    None
}

fn check_utf16(data: &[u8]) -> Option<DetectionResult> {
    let mut sample_len = data.len().min(SAMPLE_SIZE);
    sample_len -= sample_len % 2;
    if sample_len < MIN_BYTES_UTF16 {
        return None;
    }
    let num_units = sample_len / 2;
    let be_null_count = (0..sample_len).step_by(2).filter(|&i| data[i] == 0).count();
    let le_null_count = (1..sample_len).step_by(2).filter(|&i| data[i] == 0).count();
    let be_frac = be_null_count as f64 / num_units as f64;
    let le_frac = le_null_count as f64 / num_units as f64;

    let le_qualified = le_frac >= UTF16_MIN_NULL_FRACTION
        && !is_null_separator_pattern(&data[..sample_len], le_frac);
    let be_qualified = be_frac >= UTF16_MIN_NULL_FRACTION
        && !is_null_separator_pattern(&data[..sample_len], be_frac);

    if !(le_qualified || be_qualified) {
        return None;
    }

    let mut sides: Vec<(&str, f64, bool)> = vec![
        ("utf-16-le", le_frac, le_qualified),
        ("utf-16-be", be_frac, be_qualified),
    ];
    if be_frac > le_frac {
        sides.reverse();
    }

    let mut best_encoding: Option<&str> = None;
    let mut best_quality = -2.0f64;
    let mut best_qualified = false;
    let mut viable = 0usize;
    let mut qualified_side_decoded = false;

    for (encoding, _frac, qualified) in &sides {
        let text = match decode_utf16(&data[..sample_len], *encoding == "utf-16-le") {
            Some(t) => t,
            None => continue,
        };
        if *qualified {
            qualified_side_decoded = true;
        }
        if !looks_like_text(&text) {
            continue;
        }
        viable += 1;
        let quality = text_quality(&text, 500);
        let margin = if viable > 1 { QUALITY_TIE_MARGIN } else { 0.0 };
        if quality > best_quality + margin {
            best_quality = quality;
            best_encoding = Some(encoding);
            best_qualified = *qualified;
        }
    }

    let best_encoding = best_encoding?;
    let accepted = if best_qualified {
        viable == 1 || best_quality >= MIN_TEXT_QUALITY
    } else {
        qualified_side_decoded && best_quality >= MIN_TEXT_QUALITY
    };
    if accepted {
        Some(DetectionResult::enc(
            best_encoding,
            crate::utils::DETERMINISTIC_CONFIDENCE,
        ))
    } else {
        None
    }
}

/// Detect UTF-32 or UTF-16 encoding from null-byte patterns.
pub fn detect_utf1632_patterns(data: &[u8]) -> Option<DetectionResult> {
    let sample = &data[..data.len().min(SAMPLE_SIZE)];
    if sample.len() < MIN_BYTES_UTF16 {
        return None;
    }
    if let Some(r) = check_utf32(sample) {
        return Some(r);
    }
    check_utf16(sample)
}
