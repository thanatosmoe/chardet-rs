//! Stage 1b: charset declaration extraction (HTML/XML/PEP 263) and promotion.

use crate::codecs::{decode_byte, decodes_without_error};
use crate::enums::Era;
use crate::pipeline::structural::compute_structural_score;
use crate::registry::{lookup_encoding, REGISTRY_ENTRIES};
use crate::result::{DetectionResult, PipelineContext};
use crate::utils::DETERMINISTIC_CONFIDENCE;
use once_cell::sync::Lazy;
use regex::bytes::{Regex, RegexBuilder};

const SCAN_LIMIT: usize = 4096;

static XML_ENCODING_RE: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"<\?xml[^>]+encoding[ \t\r\n\f\v]*=[ \t\r\n\f\v]*['"]([^'"]+)['"]"#)
        .case_insensitive(true)
        .build()
        .unwrap()
});
static HTML5_CHARSET_RE: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"<meta[^>]+charset[ \t\r\n\f\v]*=[ \t\r\n\f\v]*['"]?[ \t\r\n\f\v]*([^ \t\r\n\f\v'">;]+)"#)
        .case_insensitive(true)
        .build()
        .unwrap()
});
static HTML4_CONTENT_TYPE_RE: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r#"<meta[^>]+content[ \t\r\n\f\v]*=[ \t\r\n\f\v]*['"][^'"]*charset=([^ \t\r\n\f\v'">;]+)"#)
        .case_insensitive(true)
        .build()
        .unwrap()
});
static PEP263_RE: Lazy<Regex> = Lazy::new(|| {
    RegexBuilder::new(r"^[ \t\x0c]*#.*?coding[:=][ \t]*([-A-Za-z0-9_.]+)")
        .multi_line(true)
        .build()
        .unwrap()
});

static EBCDIC_TAG_RE: Lazy<regex::Regex> = Lazy::new(|| {
    regex::RegexBuilder::new(r"<(?:meta|\?xml)[^>]*")
        .case_insensitive(true)
        .build()
        .unwrap()
});
static EBCDIC_DECL_RE: Lazy<regex::Regex> = Lazy::new(|| {
    regex::RegexBuilder::new(
        r"(?:charset|encoding)\s*=\s*[^\sA-Za-z0-9._-]?\s*([A-Za-z][A-Za-z0-9._-]+)",
    )
    .case_insensitive(true)
    .build()
    .unwrap()
});

fn decode_cp037_replace(head: &[u8]) -> String {
    let mut s = String::with_capacity(head.len());
    for &b in head {
        match decode_byte("cp1140", b) {
            Some(c) => s.push(c),
            None => s.push('\u{FFFD}'),
        }
    }
    s
}

fn detect_ebcdic_declaration(head: &[u8]) -> Option<DetectionResult> {
    let high_count = head.iter().filter(|&&b| b >= 0x80).count();
    if (high_count as f64) < head.len() as f64 * 0.25 {
        return None;
    }
    let decoded = decode_cp037_replace(head);
    for tag in EBCDIC_TAG_RE.find_iter(&decoded) {
        for m in EBCDIC_DECL_RE.captures_iter(tag.as_str()) {
            let name = m.get(1).unwrap().as_str().trim();
            if let Some(encoding) = lookup_encoding(name) {
                let era = REGISTRY_ENTRIES
                    .iter()
                    .find(|e| e.name == encoding)
                    .map(|e| e.era)
                    .unwrap_or(0);
                if era & Era::MAINFRAME != 0 && decodes_without_error(head, encoding) {
                    return Some(DetectionResult::with_mime(
                        Some(encoding.to_string()),
                        DETERMINISTIC_CONFIDENCE,
                        None,
                        Some("text/html".to_string()),
                    ));
                }
            }
        }
    }
    None
}

fn detect_pep263(data: &[u8]) -> Option<DetectionResult> {
    let head = &data[..data.len().min(200)];
    if !head.contains(&b'#') {
        return None;
    }
    let mut parts: Vec<&[u8]> = Vec::new();
    let mut start = 0usize;
    while parts.len() < 2 {
        match data[start..].iter().position(|&b| b == b'\n') {
            Some(p) => {
                parts.push(&data[start..start + p]);
                start = start + p + 1;
            }
            None => {
                parts.push(&data[start..]);
                break;
            }
        }
    }
    let mut joined: Vec<u8> = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            joined.push(b'\n');
        }
        joined.extend_from_slice(p);
    }
    if let Some(caps) = PEP263_RE.captures(&joined) {
        let raw_name = match std::str::from_utf8(caps.get(1).unwrap().as_bytes()) {
            Ok(s) => s.trim(),
            Err(_) => return None,
        };
        if let Some(encoding) = lookup_encoding(raw_name) {
            if validate_bytes(data, encoding) {
                return Some(DetectionResult::with_mime(
                    Some(encoding.to_string()),
                    DETERMINISTIC_CONFIDENCE,
                    None,
                    Some("text/x-python".to_string()),
                ));
            }
        }
    }
    None
}

fn validate_bytes(data: &[u8], encoding: &str) -> bool {
    decodes_without_error(&data[..data.len().min(SCAN_LIMIT)], encoding)
}

/// Scan the first bytes of `data` for a charset declaration.
pub fn detect_markup_charset(data: &[u8]) -> Option<DetectionResult> {
    if data.is_empty() {
        return None;
    }
    let head = &data[..data.len().min(SCAN_LIMIT)];

    let checks: [(&Regex, &str); 3] = [
        (&XML_ENCODING_RE, "text/xml"),
        (&HTML5_CHARSET_RE, "text/html"),
        (&HTML4_CONTENT_TYPE_RE, "text/html"),
    ];
    for (pattern, mime_type) in checks {
        if let Some(caps) = pattern.captures(head) {
            let raw_name = match std::str::from_utf8(caps.get(1).unwrap().as_bytes()) {
                Ok(s) => s.trim(),
                Err(_) => continue,
            };
            if let Some(encoding) = lookup_encoding(raw_name) {
                if validate_bytes(data, encoding) {
                    return Some(DetectionResult::with_mime(
                        Some(encoding.to_string()),
                        DETERMINISTIC_CONFIDENCE,
                        None,
                        Some(mime_type.to_string()),
                    ));
                }
            }
        }
    }

    if let Some(r) = detect_ebcdic_declaration(head) {
        return Some(r);
    }

    detect_pep263(data)
}

fn era_of_name(name: &str) -> u8 {
    REGISTRY_ENTRIES
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.era)
        .unwrap_or(0)
}

/// Promote a markup-declared encoding to its superset when structure supports it.
pub fn promote_markup_superset(
    data: &[u8],
    markup_result: DetectionResult,
    allowed: &[&str],
) -> DetectionResult {
    let enc = match &markup_result.encoding {
        Some(e) => e.clone(),
        None => return markup_result,
    };
    let (reported_codec, superset_name): (&str, &str) = match enc.as_str() {
        "shift_jis_2004" => ("shift_jis", "cp932"),
        "euc_kr" => ("euc_kr", "cp949"),
        _ => return markup_result,
    };
    if !allowed.contains(&superset_name) {
        return markup_result;
    }
    if !decodes_without_error(data, superset_name) {
        return markup_result;
    }
    if !decodes_without_error(data, reported_codec) {
        return DetectionResult::with_mime(
            Some(superset_name.to_string()),
            markup_result.confidence,
            markup_result.language.clone(),
            markup_result.mime_type.clone(),
        );
    }
    let head = &data[..data.len().min(SCAN_LIMIT)];
    let mut ctx = PipelineContext::default();
    let base_info = REGISTRY_ENTRIES
        .iter()
        .find(|e| e.name == enc)
        .expect("declared encoding is registered");
    let superset_info = REGISTRY_ENTRIES
        .iter()
        .find(|e| e.name == superset_name)
        .expect("superset is registered");
    let base_score = compute_structural_score(head, base_info, &mut ctx);
    let superset_score = compute_structural_score(head, superset_info, &mut ctx);
    if superset_score > base_score {
        return DetectionResult::with_mime(
            Some(superset_name.to_string()),
            markup_result.confidence,
            markup_result.language.clone(),
            markup_result.mime_type.clone(),
        );
    }
    let _ = era_of_name(&enc);
    markup_result
}
