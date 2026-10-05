//! Early detection of escape-sequence-based encodings (ISO-2022, HZ-GB-2312, UTF-7).

use crate::codecs::decodes_without_error;
use crate::result::DetectionResult;
use crate::utils::{count_deleted, DETERMINISTIC_CONFIDENCE, EVIDENCE_CAP_BYTES};

const HZ_GB_BYTES: &[u8] = &[
    0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F, 0x30,
    0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40,
    0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50,
    0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60,
    0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70,
    0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E,
];

fn find_range(haystack: &[u8], needle: &[u8], start: usize, end: usize) -> Option<usize> {
    if start > haystack.len() {
        return None;
    }
    let end = end.min(haystack.len());
    haystack[start..end]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + start)
}

fn find_from(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    find_range(haystack, needle, start, haystack.len())
}

fn has_valid_hz_regions(data: &[u8], max_start: usize, max_end: usize) -> bool {
    let mut start = 0usize;
    loop {
        let begin = match find_from(data, b"~{", start) {
            Some(b) if b < max_start => b,
            _ => return false,
        };
        let end = match find_range(data, b"~}", begin + 2, max_end) {
            Some(e) => e,
            None => return false,
        };
        let region = &data[begin + 2..end];
        if region.len() >= 2
            && region.len() % 2 == 0
            && count_deleted(region, HZ_GB_BYTES) == region.len()
        {
            return true;
        }
        start = end + 2;
    }
}

const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const EMBEDDED_B64_RUN: usize = 4;

fn b64_value(c: u8) -> Option<u8> {
    B64_CHARS.iter().position(|&x| x == c).map(|i| i as u8)
}

fn is_valid_utf7_b64(b64: &[u8]) -> bool {
    let n = b64.len();
    let total_bits = n * 6;
    let padding_bits = total_bits % 16;
    if padding_bits > 0 {
        let last_val = match b64_value(b64[n - 1]) {
            Some(v) => v,
            None => return false,
        };
        let mask = (1u32 << padding_bits) - 1;
        if (last_val as u32) & mask != 0 {
            return false;
        }
    }
    let num_bytes = total_bits / 8;
    let mut raw = vec![0u8; num_bytes];
    let mut bit_buf: u32 = 0;
    let mut bit_count: u32 = 0;
    let mut out_idx = 0usize;
    for &c in b64 {
        bit_buf = (bit_buf << 6) | (b64_value(c).unwrap_or(0) as u32);
        bit_count += 6;
        if bit_count >= 8 {
            bit_count -= 8;
            raw[out_idx] = ((bit_buf >> bit_count) & 0xFF) as u8;
            out_idx += 1;
        }
    }
    let mut prev_high = false;
    let mut i = 0;
    while i + 1 < num_bytes {
        let code_unit = ((raw[i] as u16) << 8) | raw[i + 1] as u16;
        if (0xD800..=0xDBFF).contains(&code_unit) {
            if prev_high {
                return false;
            }
            prev_high = true;
        } else if (0xDC00..=0xDFFF).contains(&code_unit) {
            if !prev_high {
                return false;
            }
            prev_high = false;
        } else {
            if prev_high {
                return false;
            }
            prev_high = false;
        }
        i += 2;
    }
    !prev_high
}

fn is_embedded_in_base64(data: &[u8], pos: usize) -> bool {
    let mut count = 0usize;
    let mut i = pos as isize - 1;
    while i >= 0 {
        let b = data[i as usize];
        if b == 0x0A || b == 0x0D {
            i -= 1;
            continue;
        }
        if b64_value(b).is_some() || b == b'=' {
            count += 1;
            if count >= EMBEDDED_B64_RUN {
                return true;
            }
            i -= 1;
        } else {
            break;
        }
    }
    false
}

fn single_unit(b64: &[u8]) -> u16 {
    ((b64_value(b64[0]).unwrap() as u32) << 12
        | (b64_value(b64[1]).unwrap() as u32) << 6
        | b64_value(b64[2]).unwrap() as u32)
        .wrapping_shr(2) as u16
}

fn plausible_lone_unit(unit: u16) -> bool {
    let u = unit as u32;
    (0x0080..=0x07FF).contains(&u)
        || (0x0E00..=0x0FFF).contains(&u)
        || (0x2000..=0x2BFF).contains(&u)
        || (0x3000..=0x30FF).contains(&u)
        || (0x4E00..=0x9FFF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xFF00..=0xFFEF).contains(&u)
}

fn is_b64(c: u8) -> bool {
    b64_value(c).is_some()
}

fn is_upper(c: u8) -> bool {
    c.is_ascii_uppercase()
}

fn has_valid_utf7_sequences(data: &[u8], max_start: usize, max_end: usize) -> bool {
    let mut start = 0usize;
    let limit = data.len().min(max_end);
    loop {
        let shift_pos = match find_from(data, b"+", start) {
            Some(p) if p < max_start => p,
            _ => return false,
        };
        let mut pos = shift_pos + 1;
        if pos < data.len() && data[pos] == b'-' {
            start = pos + 1;
            continue;
        }
        if pos < data.len() && data[pos] == b'+' {
            while pos < data.len() && data[pos] == b'+' {
                pos += 1;
            }
            start = pos;
            continue;
        }
        if is_embedded_in_base64(data, shift_pos) {
            start = pos;
            continue;
        }
        let mut i = pos;
        while i < limit && is_b64(data[i]) {
            i += 1;
        }
        if i == limit && limit < data.len() && is_b64(data[i]) {
            start = i;
            continue;
        }
        let b64_len = i - pos;
        let b64_data = &data[pos..i];
        if b64_len >= 3 && !b64_data.iter().any(|&b| is_upper(b)) {
            start = i;
            continue;
        }
        if b64_len >= 3 && is_valid_utf7_b64(b64_data) {
            if (b64_len * 6) / 16 == 1 {
                let unit = single_unit(b64_data);
                if !plausible_lone_unit(unit) {
                    start = i;
                    continue;
                }
            }
            return true;
        }
        start = pos.max(i);
    }
}

/// Detect ISO-2022, HZ-GB-2312, and UTF-7 from escape/tilde/plus sequences.
pub fn detect_escape_encoding(data: &[u8]) -> Option<DetectionResult> {
    let has_esc = data.contains(&0x1b);
    let has_tilde = data.contains(&b'~');
    let has_plus = data.contains(&b'+');
    if !has_esc && !has_tilde && !has_plus {
        return None;
    }

    if has_esc {
        if data.windows(4).any(|w| w == b"\x1b$(O")
            || data.windows(4).any(|w| w == b"\x1b$(P")
            || data.windows(4).any(|w| w == b"\x1b$(Q")
        {
            return Some(DetectionResult::new(
                Some("iso2022_jp_2004".into()),
                DETERMINISTIC_CONFIDENCE,
                Some("ja".into()),
            ));
        }
        if data.windows(3).any(|w| w == b"\x1b(I") {
            return Some(DetectionResult::new(
                Some("iso2022_jp_ext".into()),
                DETERMINISTIC_CONFIDENCE,
                Some("ja".into()),
            ));
        }
        if data.windows(3).any(|w| w == b"\x1b$B")
            || data.windows(3).any(|w| w == b"\x1b$@")
            || data.windows(3).any(|w| w == b"\x1b(J")
            || data.windows(4).any(|w| w == b"\x1b$(D")
        {
            if data.contains(&0x0e) && data.contains(&0x0f) {
                return Some(DetectionResult::new(
                    Some("iso2022_jp_ext".into()),
                    DETERMINISTIC_CONFIDENCE,
                    Some("ja".into()),
                ));
            }
            return Some(DetectionResult::new(
                Some("iso2022_jp_2".into()),
                DETERMINISTIC_CONFIDENCE,
                Some("ja".into()),
            ));
        }
        if data.windows(4).any(|w| w == b"\x1b$)C") {
            return Some(DetectionResult::new(
                Some("iso2022_kr".into()),
                DETERMINISTIC_CONFIDENCE,
                Some("ko".into()),
            ));
        }
    }

    let max_start = data.len().min(EVIDENCE_CAP_BYTES);
    let max_end = data.len().min(2 * EVIDENCE_CAP_BYTES);

    if has_tilde
        && data.windows(2).any(|w| w == b"~{")
        && data.windows(2).any(|w| w == b"~}")
        && has_valid_hz_regions(data, max_start, max_end)
    {
        return Some(DetectionResult::new(
            Some("hz".into()),
            DETERMINISTIC_CONFIDENCE,
            Some("zh".into()),
        ));
    }

    if has_plus
        && data.iter().all(|&b| b < 0x80)
        && has_valid_utf7_sequences(data, max_start, max_end)
        && decodes_without_error(data, "utf-7")
    {
        return Some(DetectionResult::new(
            Some("utf-7".into()),
            DETERMINISTIC_CONFIDENCE,
            None,
        ));
    }

    None
}
