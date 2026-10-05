//! End-to-end detection tests with expected values captured from Python
//! chardet 7 (so a regression here means a divergence from the reference).

use chardet::{detect, detect_all, DetectOptions, DetectionResult};

fn check(name: &str, data: &[u8], enc: Option<&str>, conf: f64, lang: Option<&str>, mime: &str) {
    let r = detect(data);
    assert_eq!(
        r.encoding.as_deref(),
        enc,
        "{name}: encoding mismatch (got {r:?})"
    );
    assert!(
        (r.confidence - conf).abs() < 1e-12,
        "{name}: confidence {} != {conf}",
        r.confidence
    );
    assert_eq!(r.language.as_deref(), lang, "{name}: language mismatch");
    assert_eq!(r.mime_type.as_deref(), Some(mime), "{name}: mime mismatch");
}

#[test]
fn empty_input() {
    check("empty", b"", Some("utf-8"), 0.1, None, "text/plain");
}

#[test]
fn ascii() {
    check(
        "ascii",
        b"Hello, world!",
        Some("ascii"),
        1.0,
        Some("pl"),
        "text/plain",
    );
}

#[test]
fn utf8() {
    let data = "café ☕ — произведение".as_bytes();
    check("utf8", data, Some("utf-8"), 0.99, Some("ru"), "text/plain");
}

#[test]
fn utf8_bom() {
    let mut data = vec![0xEF, 0xBB, 0xBF];
    data.extend_from_slice(b"Hello");
    check(
        "utf8sig",
        &data,
        Some("UTF-8-SIG"),
        1.0,
        Some("fi"),
        "text/plain",
    );
}

#[test]
fn utf16_bom() {
    let data: Vec<u8> = "Hello UTF-16"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let mut with_bom = vec![0xFF, 0xFE];
    with_bom.extend_from_slice(&data);
    check(
        "utf16",
        &with_bom,
        Some("UTF-16"),
        1.0,
        Some("it"),
        "text/plain",
    );
}

#[test]
fn utf32_bom() {
    let mut data = vec![0xFF, 0xFE, 0x00, 0x00];
    for c in "Hello UTF-32".chars() {
        data.extend_from_slice(&(c as u32).to_le_bytes());
    }
    check(
        "utf32",
        &data,
        Some("UTF-32"),
        1.0,
        Some("it"),
        "text/plain",
    );
}

#[test]
fn windows_1251() {
    // "Съешь же ещё этих мягких французских булок, да выпей чаю." in cp1251.
    let hex = "d1fae5f8fc20e6e520e5f9b820fdf2e8f520ecffe3eae8f520f4f0e0edf6f3e7f1eae8f520e1f3ebeeea2c20e4e020e2fbefe5e920f7e0fe2e";
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    check(
        "cp1251",
        &bytes,
        Some("Windows-1251"),
        0.20519701419221748,
        Some("ru"),
        "text/plain",
    );
}

#[test]
fn binary_is_none() {
    let data: Vec<u8> = (0..=255u8).collect();
    check(
        "binary",
        &data,
        None,
        0.95,
        None,
        "application/octet-stream",
    );
}

#[test]
fn png_magic() {
    let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
    data.extend_from_slice(&[0u8; 100]);
    check("png", &data, None, 1.0, None, "image/png");
}

#[test]
fn html_meta_charset() {
    let data = b"<html><head><meta charset=\"utf-8\"></head><body>hi</body></html>";
    check(
        "html",
        data,
        Some("utf-8"),
        0.95,
        Some("en"),
        "text/html",
    );
}

#[test]
fn detect_all_returns_ranked_results() {
    // "Le café ..." in cp1252.
    let data = "Le café est une boisson très populaire en France et dans le monde entier."
        .chars()
        .map(|c| match c {
            'é' => 0xE9,
            'è' => 0xE8,
            'à' => 0xE0,
            _ => c as u8,
        })
        .collect::<Vec<u8>>();
    let opts = DetectOptions::default();
    let results = detect_all(&data, &opts).unwrap();
    assert!(!results.is_empty());
    // Sorted descending.
    for w in results.windows(2) {
        assert!(w[0].confidence >= w[1].confidence);
    }
    assert_eq!(results[0].encoding.as_deref(), Some("Windows-1252"));
    assert!((results[0].confidence - 0.3117672619507463).abs() < 1e-12);
}

#[test]
fn deterministic_across_calls() {
    let data = "日本語の文字コード検出テストです。".as_bytes();
    let a = detect(data);
    let b = detect(data);
    assert_eq!(a, b);
}

#[test]
fn markup_superset_promotion_shift_jis_to_cp932() {
    // A page declaring shift_jis but using CP932-only glyphs must promote.
    let hex = "3c68746d6c3e3c6d65746120636861727365743d2273686966745f6a6973223e8754875587563c2f68746d6c3e";
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    check(
        "shiftjis_promote",
        &bytes,
        Some("CP932"),
        0.95,
        Some("ja"),
        "text/html",
    );
}

#[test]
fn detection_result_type_is_send() {
    // Sanity: results are plain data and can cross threads.
    let r = DetectionResult::enc("utf-8", 0.99);
    std::thread::spawn(move || assert_eq!(r.encoding.as_deref(), Some("utf-8")))
        .join()
        .unwrap();
}
