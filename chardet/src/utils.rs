//! Shared constants and helpers, mirroring `chardet._utils`.

/// Default maximum number of bytes examined during detection.
pub const DEFAULT_MAX_BYTES: usize = 200_000;

/// Evidence cap for the filtering/validation/probing stages (ADR-0006).
pub const EVIDENCE_CAP_BYTES: usize = 256 * 1024;

/// Confidence for deterministic (non-BOM) detection stages.
pub const DETERMINISTIC_CONFIDENCE: f64 = 0.95;

/// Default minimum confidence threshold for filtering results.
pub const MINIMUM_THRESHOLD: f64 = 0.20;

/// Bytes considered valid in ASCII text: tab, LF, CR, and printable ASCII.
pub const ASCII_TEXT_BYTES: &[u8] = &[
    0x09, 0x0A, 0x0D, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B,
    0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A,
    0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
    0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67,
    0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76,
    0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E,
];

/// Mapping from ISO 639-1 language codes to English names.
pub fn language_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "ar" => "arabic",
        "be" => "belarusian",
        "bg" => "bulgarian",
        "br" => "breton",
        "cs" => "czech",
        "cy" => "welsh",
        "da" => "danish",
        "de" => "german",
        "el" => "greek",
        "en" => "english",
        "eo" => "esperanto",
        "es" => "spanish",
        "et" => "estonian",
        "fa" => "farsi",
        "fi" => "finnish",
        "fr" => "french",
        "ga" => "irish",
        "gd" => "gaelic",
        "he" => "hebrew",
        "hr" => "croatian",
        "hu" => "hungarian",
        "id" => "indonesian",
        "is" => "icelandic",
        "it" => "italian",
        "ja" => "japanese",
        "kk" => "kazakh",
        "ko" => "korean",
        "lt" => "lithuanian",
        "lv" => "latvian",
        "mk" => "macedonian",
        "ms" => "malay",
        "mt" => "maltese",
        "nl" => "dutch",
        "no" => "norwegian",
        "pl" => "polish",
        "pt" => "portuguese",
        "ro" => "romanian",
        "ru" => "russian",
        "sk" => "slovak",
        "sl" => "slovene",
        "sr" => "serbian",
        "sv" => "swedish",
        "tg" => "tajik",
        "th" => "thai",
        "tr" => "turkish",
        "uk" => "ukrainian",
        "und" => "undetermined",
        "ur" => "urdu",
        "vi" => "vietnamese",
        "zh" => "chinese",
        _ => return None,
    })
}

/// Count how many bytes of `data` are present in `table`.
pub fn count_deleted(data: &[u8], table: &[u8]) -> usize {
    let mut present = [false; 256];
    for &b in table {
        present[b as usize] = true;
    }
    data.iter().filter(|&&b| present[b as usize]).count()
}
