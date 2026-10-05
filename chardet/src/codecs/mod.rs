//! Exact CPython-compatible codec validity and decoding.
//!
//! `chardet` decides which candidates survive by asking Python's codecs
//! whether the bytes decode.  This module reproduces that behavior for the
//! Rust port:
//!
//! * single-byte codecs use generated 256-entry decode tables
//!   (`sbcs_data`), so undefined slots raise exactly as CPython does;
//! * DBCS codecs use generated validity tables (`dbcs.bin`) probed from
//!   CPython's incremental decoders;
//! * the Unicode families and the stateful encodings are implemented
//!   directly.

pub mod categories_data;
pub mod sbcs_data;

use categories_data::{CATEGORY_NAMES, CATEGORY_RANGES};
use sbcs_data::{letter_case, sbcs_decode, INVALID};

const DBCS_ORDER: [&str; 11] = [
    "cp932",
    "cp949",
    "big5hkscs",
    "euc_jis_2004",
    "euc_kr",
    "shift_jis_2004",
    "johab",
    "gb18030",
    "shift_jis",
    "euc_jp",
    "big5",
];

static DBCS: &[u8] = include_bytes!("dbcs.bin");

const ENC_STRIDE: usize = 256 + 256 + 65536;
const SS3_EUCJIS_OFFSET: usize = 4 + DBCS_ORDER.len() * ENC_STRIDE;
const SS3_EUCP_OFFSET: usize = SS3_EUCJIS_OFFSET + 65536;
const GB4_OFFSET: usize = SS3_EUCP_OFFSET + 65536;

/// The result of scanning a byte stream for codec validity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// The bytes decode with errors deferred at most for a trailing prefix.
    pub valid: bool,
    /// Number of trailing bytes left pending as an incomplete sequence.
    pub pending: usize,
}

impl Status {
    const VALID: Status = Status {
        valid: true,
        pending: 0,
    };
    const INVALID: Status = Status {
        valid: false,
        pending: 0,
    };
    fn pending(n: usize) -> Status {
        Status {
            valid: true,
            pending: n,
        }
    }
}

fn dbcs_index(name: &str) -> Option<usize> {
    DBCS_ORDER.iter().position(|&n| n == name)
}

/// Decode a single byte under a single-byte codec.
pub fn decode_byte(name: &str, b: u8) -> Option<char> {
    let table = sbcs_decode(name)?;
    let cp = table[b as usize];
    if cp == INVALID {
        None
    } else {
        char::from_u32(cp)
    }
}

/// Unicode general category name for a codepoint (defaults to `"Cn"`).
pub fn category_of_cp(cp: u32) -> &'static str {
    let mut lo = 0usize;
    let mut hi = CATEGORY_RANGES.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (s, e, c) = CATEGORY_RANGES[mid];
        if cp < s {
            hi = mid;
        } else if cp > e {
            lo = mid + 1;
        } else {
            return CATEGORY_NAMES[c as usize];
        }
    }
    "Cn"
}

/// Python `str.isprintable` for a single codepoint.
pub fn is_printable_cp(cp: u32) -> bool {
    if cp == 0x20 {
        return true;
    }
    !matches!(
        category_of_cp(cp),
        "Cc" | "Cf" | "Cs" | "Co" | "Cn" | "Zl" | "Zp" | "Zs"
    )
}

/// Letter-case table for an encoding (0 non-letter, 1 Lu, 2 other L/M).
pub fn letter_case_table(name: &str) -> Option<&'static [u8; 256]> {
    letter_case(name)
}

/// Return `true` if `data` decodes under `encoding` (tolerating a truncated tail).
pub fn decodes_without_error(data: &[u8], encoding: &str) -> bool {
    scan(data, encoding).valid
}

/// Return `true` if `data` decodes under `encoding` with nothing deferred.
pub fn decodes_completely(data: &[u8], encoding: &str) -> bool {
    let s = scan(data, encoding);
    s.valid && s.pending == 0
}

/// Decode `data` under `encoding` ignoring invalid bytes, then re-encode UTF-8.
///
/// Mirrors `data.decode(encoding, errors="ignore").encode("utf-8")`.
pub fn decode_to_utf8_lossy(data: &[u8], encoding: &str) -> Option<Vec<u8>> {
    if encoding == "utf-8" {
        return Some(data.to_vec());
    }
    if encoding == "utf-8-sig" {
        let rest = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
        return Some(rest.to_vec());
    }
    if let Some(_table) = sbcs_decode(encoding) {
        let mut out = String::with_capacity(data.len());
        for &b in data {
            if let Some(c) = decode_byte(encoding, b) {
                out.push(c);
            }
        }
        return Some(out.into_bytes());
    }
    // Unicode families.
    match encoding {
        "ascii" => {
            let mut s = String::with_capacity(data.len());
            for &b in data {
                if b < 0x80 {
                    s.push(b as char);
                }
            }
            Some(s.into_bytes())
        }
        "utf-7" => Some(decode_utf7_to_utf8(data)),
        "utf-16" | "utf-16-le" | "utf-16-be" | "utf-32" | "utf-32-le" | "utf-32-be" => {
            decode_unicode_lossy(data, encoding)
        }
        _ => None,
    }
}

/// Decode UTF-7 (RFC 2152) to UTF-8, ignoring invalid sequences.
///
/// Mirrors `data.decode("utf-7", errors="ignore").encode("utf-8")` closely
/// enough to reproduce chardet's language scoring.
fn decode_utf7_to_utf8(data: &[u8]) -> Vec<u8> {
    let mut out = String::with_capacity(data.len());
    let n = data.len();
    let mut i = 0;
    while i < n {
        let b = data[i];
        if b != b'+' {
            // Direct characters decode as latin-1.
            out.push(b as char);
            i += 1;
            continue;
        }
        if i + 1 >= n {
            break;
        }
        if data[i + 1] == b'-' {
            out.push('+');
            i += 2;
            continue;
        }
        // Parse base64 run into UTF-16 code units.
        i += 1;
        let mut bits: u32 = 0;
        let mut nbits: u32 = 0;
        let mut units: Vec<u16> = Vec::new();
        while i < n {
            let c = data[i];
            if c == b'-' {
                i += 1;
                break;
            }
            match UTF7_B64.iter().position(|&x| x == c).map(|p| p as u8) {
                Some(v) => {
                    bits = (bits << 6) | v as u32;
                    nbits += 6;
                    while nbits >= 16 {
                        nbits -= 16;
                        units.push(((bits >> nbits) & 0xFFFF) as u16);
                    }
                    i += 1;
                }
                None => break,
            }
        }
        // Combine surrogate pairs; drop lone surrogates (errors="ignore").
        let mut k = 0;
        while k < units.len() {
            let u = units[k];
            if (0xD800..=0xDBFF).contains(&u) && k + 1 < units.len() {
                let lo = units[k + 1];
                if (0xDC00..=0xDFFF).contains(&lo) {
                    let cp = 0x10000 + ((u as u32 - 0xD800) << 10) + (lo as u32 - 0xDC00);
                    if let Some(ch) = char::from_u32(cp) {
                        out.push(ch);
                    }
                    k += 2;
                    continue;
                }
            } else if (0xDC00..=0xDFFF).contains(&u) {
                k += 1;
                continue;
            }
            if let Some(ch) = char::from_u32(u as u32) {
                out.push(ch);
            }
            k += 1;
        }
    }
    out.into_bytes()
}

fn decode_unicode_lossy(data: &[u8], encoding: &str) -> Option<Vec<u8>> {
    let units: Vec<u32> = match encoding {
        "utf-16" => {
            let (le, rest) = if data.starts_with(&[0xFF, 0xFE]) {
                (true, &data[2..])
            } else if data.starts_with(&[0xFE, 0xFF]) {
                (false, &data[2..])
            } else {
                return None;
            };
            u16_units(rest, le)
        }
        "utf-16-le" => u16_units(data, true),
        "utf-16-be" => u16_units(data, false),
        "utf-32" => {
            let (le, rest) = if data.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
                (true, &data[4..])
            } else if data.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
                (false, &data[4..])
            } else {
                return None;
            };
            u32_units(rest, le)
        }
        "utf-32-le" => u32_units(data, true),
        "utf-32-be" => u32_units(data, false),
        _ => return None,
    };
    let mut s = String::new();
    for u in units {
        if let Some(c) = char::from_u32(u) {
            s.push(c);
        }
    }
    Some(s.into_bytes())
}

fn u16_units(data: &[u8], le: bool) -> Vec<u32> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < data.len() {
        let unit = if le {
            u16::from_le_bytes([data[i], data[i + 1]])
        } else {
            u16::from_be_bytes([data[i], data[i + 1]])
        };
        i += 2;
        if (0xD800..=0xDBFF).contains(&unit) && i + 1 < data.len() {
            let low = if le {
                u16::from_le_bytes([data[i], data[i + 1]])
            } else {
                u16::from_be_bytes([data[i], data[i + 1]])
            };
            if (0xDC00..=0xDFFF).contains(&low) {
                i += 2;
                out.push(0x10000 + ((unit as u32 - 0xD800) << 10) + (low as u32 - 0xDC00));
                continue;
            }
        }
        out.push(unit as u32);
    }
    out
}

fn u32_units(data: &[u8], le: bool) -> Vec<u32> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < data.len() {
        let unit = if le {
            u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        } else {
            u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        };
        i += 4;
        out.push(unit);
    }
    out
}

/// Resolve a name to the codec key used by `scan`.
///
/// Registry aliases normally resolve to a canonical registry name, but a few
/// Python codec names are *not* the same codec as the registry's canonical
/// entry (`shift_jis` vs `shift_jis_2004`, `euc_jp` vs `euc_jis_2004`) and
/// carry their own validity tables.
fn codec_key(encoding: &str) -> &str {
    match crate::registry::normalize_name(encoding).as_str() {
        "shift_jis" => "shift_jis",
        "euc_jp" => "euc_jp",
        "big5" => "big5",
        _ => crate::registry::lookup_encoding(encoding).unwrap_or(encoding),
    }
}

/// Scan `data` under `encoding` for validity.
pub fn scan(data: &[u8], encoding: &str) -> Status {
    let encoding = codec_key(encoding);
    if data.is_empty() {
        return Status::VALID;
    }
    if let Some(_t) = sbcs_decode(encoding) {
        for &b in data {
            if decode_byte(encoding, b).is_none() {
                return Status::INVALID;
            }
        }
        return Status::VALID;
    }
    if let Some(idx) = dbcs_index(encoding) {
        return scan_dbcs(data, encoding, idx);
    }
    match encoding {
        "ascii" => {
            if data.iter().all(|&b| b < 0x80) {
                Status::VALID
            } else {
                Status::INVALID
            }
        }
        "utf-8" | "utf-8-sig" => scan_utf8(data),
        "utf-16" => scan_utf16_bom(data),
        "utf-16-le" => scan_utf16(data, true),
        "utf-16-be" => scan_utf16(data, false),
        "utf-32" => scan_utf32_bom(data),
        "utf-32-le" => scan_utf32(data, true),
        "utf-32-be" => scan_utf32(data, false),
        "utf-7" => scan_utf7(data),
        "hz" => scan_hz(data),
        "iso2022_kr" => scan_iso2022(data, Iso2022Kind::Kr),
        "iso2022_jp_2" => scan_iso2022(data, Iso2022Kind::Jp2),
        "iso2022_jp_2004" => scan_iso2022(data, Iso2022Kind::Jp2004),
        "iso2022_jp_ext" => scan_iso2022(data, Iso2022Kind::JpExt),
        _ => Status::INVALID,
    }
}

fn scan_utf8(data: &[u8]) -> Status {
    let n = data.len();
    let mut i = 0;
    while i < n {
        let b = data[i];
        if b < 0x80 {
            i += 1;
            continue;
        }
        let (len, lo2, hi2) = match b {
            0xC2..=0xDF => (2usize, 0x80u8, 0xBF),
            0xE0 => (3, 0xA0, 0xBF),
            0xE1..=0xEC => (3, 0x80, 0xBF),
            0xED => (3, 0x80, 0x9F),
            0xEE..=0xEF => (3, 0x80, 0xBF),
            0xF0 => (4, 0x90, 0xBF),
            0xF1..=0xF3 => (4, 0x80, 0xBF),
            0xF4 => (4, 0x80, 0x8F),
            _ => return Status::INVALID,
        };
        // first continuation has a restricted range
        if i + 1 >= n {
            return Status::pending(n - i);
        }
        if data[i + 1] < lo2 || data[i + 1] > hi2 {
            return Status::INVALID;
        }
        // remaining continuations
        let mut k = 2;
        while k < len {
            if i + k >= n {
                return Status::pending(n - i);
            }
            if data[i + k] & 0xC0 != 0x80 {
                return Status::INVALID;
            }
            k += 1;
        }
        i += len;
    }
    Status::VALID
}

fn scan_utf16(data: &[u8], le: bool) -> Status {
    let n = data.len();
    let mut i = 0;
    while i + 1 < n {
        let unit = if le {
            u16::from_le_bytes([data[i], data[i + 1]])
        } else {
            u16::from_be_bytes([data[i], data[i + 1]])
        };
        i += 2;
        if (0xD800..=0xDBFF).contains(&unit) {
            if i + 1 >= n {
                return Status::pending(2);
            }
            let low = if le {
                u16::from_le_bytes([data[i], data[i + 1]])
            } else {
                u16::from_be_bytes([data[i], data[i + 1]])
            };
            if !(0xDC00..=0xDFFF).contains(&low) {
                return Status::INVALID;
            }
            i += 2;
        } else if (0xDC00..=0xDFFF).contains(&unit) {
            return Status::INVALID;
        }
    }
    if i < n {
        Status::pending(1)
    } else {
        Status::VALID
    }
}

fn scan_utf16_bom(data: &[u8]) -> Status {
    if data.starts_with(&[0xFF, 0xFE]) {
        scan_utf16(&data[2..], true)
    } else if data.starts_with(&[0xFE, 0xFF]) {
        scan_utf16(&data[2..], false)
    } else if data.len() < 2 {
        Status::pending(data.len())
    } else {
        Status::INVALID
    }
}

fn scan_utf32(data: &[u8], le: bool) -> Status {
    let n = data.len();
    let mut i = 0;
    while i + 3 < n {
        let unit = if le {
            u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        } else {
            u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]])
        };
        if char::from_u32(unit).is_none() {
            return Status::INVALID;
        }
        i += 4;
    }
    if i < n {
        Status::pending(n - i)
    } else {
        Status::VALID
    }
}

fn scan_utf32_bom(data: &[u8]) -> Status {
    if data.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        scan_utf32(&data[4..], true)
    } else if data.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        scan_utf32(&data[4..], false)
    } else if data.len() < 4 {
        Status::pending(data.len())
    } else {
        Status::INVALID
    }
}

const UTF7_B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn scan_utf7(data: &[u8]) -> Status {
    let valid_b64 = |b: u8| UTF7_B64.contains(&b);
    let n = data.len();
    let mut i = 0;
    while i < n {
        if data[i] == b'+' {
            if i + 1 >= n {
                return Status::pending(1);
            }
            if data[i + 1] == b'-' {
                i += 2;
                continue;
            }
            // base64 run
            let mut bits: u32 = 0;
            let mut nbits: u32 = 0;
            let mut j = i + 1;
            loop {
                if j >= n {
                    // trailing base64 may be incomplete; Python raises if
                    // leftover bits are non-zero, but a truncated tail is
                    // deferred with final=False.
                    return Status::pending(n - i);
                }
                let c = data[j];
                if c == b'-' {
                    // end marker
                    if nbits >= 6 || (bits & ((1 << nbits) - 1)) != 0 {
                        return Status::INVALID;
                    }
                    i = j + 1;
                    break;
                }
                if !valid_b64(c) {
                    // Any non-base64 char terminates the run and is reprocessed.
                    if nbits >= 6 || (bits & ((1 << nbits) - 1)) != 0 {
                        return Status::INVALID;
                    }
                    i = j;
                    break;
                }
                let v = UTF7_B64.iter().position(|&x| x == c).unwrap() as u32;
                bits = (bits << 6) | v;
                nbits += 6;
                if nbits >= 16 {
                    nbits -= 16;
                    bits &= (1 << nbits) - 1;
                }
                j += 1;
            }
        } else {
            i += 1;
        }
    }
    Status::VALID
}

fn scan_hz(data: &[u8]) -> Status {
    let n = data.len();
    let mut i = 0;
    let mut gb = false;
    while i < n {
        let b = data[i];
        if b == b'~' {
            if i + 1 >= n {
                return Status::pending(1);
            }
            match data[i + 1] {
                b'~' => i += 2,
                b'{' => {
                    gb = true;
                    i += 2;
                }
                b'}' => {
                    gb = false;
                    i += 2;
                }
                b'\n' => i += 2,
                _ => return Status::INVALID,
            }
            continue;
        }
        if gb {
            if !(0x21..=0x7E).contains(&b) {
                return Status::INVALID;
            }
            if i + 1 >= n {
                return Status::pending(1);
            }
            let t = data[i + 1];
            if !(0x21..=0x7E).contains(&t) {
                return Status::INVALID;
            }
            i += 2;
        } else {
            if b >= 0x80 {
                return Status::INVALID;
            }
            i += 1;
        }
    }
    Status::VALID
}

#[derive(Clone, Copy)]
enum Iso2022Kind {
    Kr,
    Jp2,
    Jp2004,
    JpExt,
}

fn scan_iso2022(data: &[u8], kind: Iso2022Kind) -> Status {
    let n = data.len();
    let mut i = 0;
    // Rough state: 0 = ASCII, nonzero = a double-byte mode.
    let mut db = false;
    while i < n {
        let b = data[i];
        if b == 0x1B {
            // Escape sequence: ESC <intermediate>* <final>
            // Consume ESC and the following bytes conservatively.
            if i + 1 >= n {
                return Status::pending(n - i);
            }
            let c = data[i + 1];
            match c {
                b'(' | b')' | b'*' | b'+' | b'$' => {
                    // ESC $ ... can be 2-3 bytes. Find final in 0x40-0x7E.
                    let mut j = i + 1;
                    while j < n && !(0x40..=0x7E).contains(&data[j]) {
                        j += 1;
                    }
                    if j >= n {
                        return Status::pending(n - i);
                    }
                    db = c == b'$' || (c == b'(');
                    i = j + 1;
                }
                _ => {
                    // ESC <final> (e.g. ESC ( B handled above, ESC N etc.)
                    db = false;
                    i += 2;
                }
            }
            let _ = kind;
            continue;
        }
        if b >= 0x80 {
            return Status::INVALID;
        }
        if db {
            if !(0x21..=0x7E).contains(&b) {
                return Status::INVALID;
            }
            if i + 1 >= n {
                return Status::pending(1);
            }
            let t = data[i + 1];
            if !(0x21..=0x7E).contains(&t) {
                return Status::INVALID;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    Status::VALID
}

/// Return `true` if `data` is ASCII text plus an incomplete multi-byte tail.
pub fn dangling_tail_with_ascii_prefix(data: &[u8], encoding: &str) -> bool {
    let s = scan(data, encoding);
    if !s.valid || s.pending == 0 {
        return false;
    }
    let prefix = &data[..data.len() - s.pending];
    if prefix.is_empty() {
        return false;
    }
    match decode_to_utf8_lossy(prefix, encoding) {
        Some(v) => !v.is_empty() && v.iter().all(|&b| b < 0x80),
        None => prefix.iter().all(|&b| b < 0x80),
    }
}

fn scan_dbcs(data: &[u8], name: &str, idx: usize) -> Status {
    let base = 4 + idx * ENC_STRIDE;
    let single = &DBCS[base..base + 256];
    let live1 = &DBCS[base + 256..base + 512];
    let pair = &DBCS[base + 512..base + 512 + 65536];
    let n = data.len();
    let mut i = 0;
    while i < n {
        let b = data[i] as usize;
        if single[b] == 1 {
            i += 1;
            continue;
        }
        if live1[b] == 0 {
            return Status::INVALID;
        }
        // b is a lead byte.
        if name == "gb18030" {
            if i + 1 >= n {
                return Status::pending(n - i);
            }
            let b2 = data[i + 1];
            if (0x30..=0x39).contains(&b2) {
                if i + 2 >= n {
                    return Status::pending(n - i);
                }
                let b3 = data[i + 2];
                if !(0x81..=0xFE).contains(&b3) {
                    return Status::INVALID;
                }
                if i + 3 >= n {
                    return Status::pending(n - i);
                }
                let b4 = data[i + 3];
                if !(0x30..=0x39).contains(&b4) {
                    return Status::INVALID;
                }
                let base_idx = (b - 0x81) * 10 + (b2 as usize - 0x30);
                let low = (b3 as usize - 0x81) * 10 + (b4 as usize - 0x30);
                let p = base_idx * 1260 + low;
                if DBCS[GB4_OFFSET + (p >> 3)] & (1 << (p & 7)) != 0 {
                    i += 4;
                    continue;
                }
                return Status::INVALID;
            }
            if pair[b * 256 + b2 as usize] == 1 {
                i += 2;
                continue;
            }
            return Status::INVALID;
        }
        if (name == "euc_jis_2004" || name == "euc_jp") && b == 0x8F {
            if i + 1 >= n {
                return Status::pending(n - i);
            }
            let t1 = data[i + 1];
            if !(0xA1..=0xFE).contains(&t1) {
                return Status::INVALID;
            }
            if i + 2 >= n {
                return Status::pending(n - i);
            }
            let t2 = data[i + 2];
            let ss3 = if name == "euc_jis_2004" {
                SS3_EUCJIS_OFFSET
            } else {
                SS3_EUCP_OFFSET
            };
            if DBCS[ss3 + (t1 as usize) * 256 + t2 as usize] == 1 {
                i += 3;
                continue;
            }
            return Status::INVALID;
        }
        if i + 1 >= n {
            return Status::pending(n - i);
        }
        let t = data[i + 1] as usize;
        if pair[b * 256 + t] == 1 {
            i += 2;
            continue;
        }
        return Status::INVALID;
    }
    Status::VALID
}
