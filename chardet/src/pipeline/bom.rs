//! Stage 1a: BOM (byte order mark) detection.

use crate::codecs::decodes_without_error;
use crate::result::DetectionResult;

const BOMS: &[(&[u8], &str)] = &[
    (b"\x00\x00\xfe\xff", "utf-32"),
    (b"\xff\xfe\x00\x00", "utf-32"),
    (b"\xef\xbb\xbf", "utf-8-sig"),
    (b"\xfe\xff", "utf-16"),
    (b"\xff\xfe", "utf-16"),
    (b"+/v8", "utf-7"),
    (b"+/v9", "utf-7"),
    (b"+/v+", "utf-7"),
    (b"+/v/", "utf-7"),
];

fn is_utf32_bom(b: &[u8]) -> bool {
    b == b"\x00\x00\xfe\xff" || b == b"\xff\xfe\x00\x00"
}

/// Check for a byte order mark at the start of `data`.
pub fn detect_bom(data: &[u8]) -> Option<DetectionResult> {
    for (bom, encoding) in BOMS {
        if data.starts_with(bom) {
            if is_utf32_bom(bom) {
                let payload_len = data.len() - bom.len();
                if payload_len % 4 != 0 {
                    continue;
                }
            }
            if *encoding == "utf-7" && !decodes_without_error(data, "utf-7") {
                continue;
            }
            return Some(DetectionResult::enc(encoding, 1.0));
        }
    }
    None
}
