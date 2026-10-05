//! Magic number detection for binary file types.

use crate::result::DetectionResult;

const MAGIC_NUMBERS: &[(&[u8], &str)] = &[
    (b"\x89PNG\r\n\x1a\n", "image/png"),
    (b"GIF87a", "image/gif"),
    (b"GIF89a", "image/gif"),
    (b"MM\x00\x2a", "image/tiff"),
    (b"II\x2a\x00", "image/tiff"),
    (b"8BPS", "image/vnd.adobe.photoshop"),
    (b"qoif", "image/qoi"),
    (b"BM", "image/bmp"),
    (b"\xff\xd8\xff", "image/jpeg"),
    (
        b"\x00\x00\x00\x0c\x4a\x58\x4c\x20\x0d\x0a\x87\x0a",
        "image/jxl",
    ),
    (b"\xff\x0a", "image/jxl"),
    (b"\x00\x00\x01\x00", "image/vnd.microsoft.icon"),
    (b"ID3", "audio/mpeg"),
    (b"MThd", "audio/midi"),
    (b"OggS", "audio/ogg"),
    (b"fLaC", "audio/flac"),
    (b"\x1a\x45\xdf\xa3", "video/webm"),
    (b"\x1f\x8b", "application/gzip"),
    (b"BZh", "application/x-bzip2"),
    (b"\xfd7zXZ\x00", "application/x-xz"),
    (b"7z\xbc\xaf\x27\x1c", "application/x-7z-compressed"),
    (b"Rar!\x1a\x07\x01\x00", "application/vnd.rar"),
    (b"Rar!\x1a\x07\x00", "application/vnd.rar"),
    (b"\x28\xb5\x2f\xfd", "application/zstd"),
    (b"%PDF-", "application/pdf"),
    (b"SQLite format 3\x00", "application/x-sqlite3"),
    (b"ARROW1", "application/vnd.apache.arrow.file"),
    (b"PAR1", "application/vnd.apache.parquet"),
    (b"\x00asm", "application/wasm"),
    (b"dex\n", "application/vnd.android.dex"),
    (b"\x7fELF", "application/x-elf"),
    (b"\xfe\xed\xfa\xce", "application/x-mach-binary"),
    (b"\xfe\xed\xfa\xcf", "application/x-mach-binary"),
    (b"\xce\xfa\xed\xfe", "application/x-mach-binary"),
    (b"\xcf\xfa\xed\xfe", "application/x-mach-binary"),
    (b"MZ", "application/vnd.microsoft.portable-executable"),
    (b"wOFF", "font/woff"),
    (b"wOF2", "font/woff2"),
    (b"OTTO", "font/otf"),
    (b"\x00\x01\x00\x00", "font/ttf"),
];

const TAR_OFFSET: usize = 257;
const TAR_SIGNATURES: &[&[u8]] = &[b"ustar\x00", b"ustar "];

fn riff_subtype(tag: &[u8]) -> Option<&'static str> {
    Some(match tag {
        b"WEBP" => "image/webp",
        b"WAVE" => "audio/wav",
        b"AVI " => "video/x-msvideo",
        _ => return None,
    })
}

fn form_subtype(tag: &[u8]) -> Option<&'static str> {
    Some(match tag {
        b"AIFF" | b"AIFC" => "audio/aiff",
        _ => return None,
    })
}

const ZIP_SIGNATURE: &[u8] = b"PK\x03\x04";
const ZIP_SCAN_LIMIT: usize = 4096;

const ZIP_FILENAME_PREFIXES: &[(&[u8], &str)] = &[
    (
        b"xl/",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    (
        b"word/",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (
        b"ppt/",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    (b"META-INF/MANIFEST.MF", "application/java-archive"),
    (
        b"AndroidManifest.xml",
        "application/vnd.android.package-archive",
    ),
    (b"META-INF/container.xml", "application/epub+zip"),
];

const ZIP_FILENAME_SUFFIXES: &[(&[u8], &str)] = &[(b".dist-info/", "application/x-wheel+zip")];

const OPENDOCUMENT_MIMES: &[&[u8]] = &[
    b"application/vnd.oasis.opendocument.text",
    b"application/vnd.oasis.opendocument.spreadsheet",
    b"application/vnd.oasis.opendocument.presentation",
    b"application/vnd.oasis.opendocument.graphics",
];

fn le16(d: &[u8], o: usize) -> usize {
    (d[o] as usize) | ((d[o + 1] as usize) << 8)
}
fn le32(d: &[u8], o: usize) -> usize {
    (d[o] as usize)
        | ((d[o + 1] as usize) << 8)
        | ((d[o + 2] as usize) << 16)
        | ((d[o + 3] as usize) << 24)
}

fn classify_zip(data: &[u8]) -> &'static str {
    let scan = &data[..data.len().min(ZIP_SCAN_LIMIT)];
    let mut offset = 0usize;
    loop {
        let idx = match find(scan, ZIP_SIGNATURE, offset) {
            Some(i) if scan.len() >= i + 30 => i,
            _ => break,
        };
        let name_len = le16(scan, idx + 26);
        let extra_len = le16(scan, idx + 28);
        let name_start = idx + 30;
        if scan.len() < name_start + name_len {
            break;
        }
        let name = &scan[name_start..name_start + name_len];
        for (prefix, mime) in ZIP_FILENAME_PREFIXES {
            if name.starts_with(prefix) {
                return mime;
            }
        }
        for (suffix, mime) in ZIP_FILENAME_SUFFIXES {
            if contains(name, suffix) {
                return mime;
            }
        }
        if name == b"mimetype" {
            let compression = le16(scan, idx + 8);
            if compression == 0 {
                let content_start = name_start + name_len + extra_len;
                let content_len = le32(scan, idx + 22);
                if scan.len() >= content_start + content_len {
                    let content = &scan[content_start..content_start + content_len];
                    if let Some(mime) = OPENDOCUMENT_MIMES.iter().find(|&&m| m == content) {
                        // SAFETY of lifetime: strings are static.
                        return match *mime {
                            b"application/vnd.oasis.opendocument.text" => {
                                "application/vnd.oasis.opendocument.text"
                            }
                            b"application/vnd.oasis.opendocument.spreadsheet" => {
                                "application/vnd.oasis.opendocument.spreadsheet"
                            }
                            b"application/vnd.oasis.opendocument.presentation" => {
                                "application/vnd.oasis.opendocument.presentation"
                            }
                            _ => "application/vnd.oasis.opendocument.graphics",
                        };
                    }
                }
            }
        }
        let flags = le16(scan, idx + 6);
        let content_size = if flags & 0x0008 != 0 {
            0
        } else {
            le32(scan, idx + 18)
        };
        offset = name_start + name_len + extra_len + content_size;
    }
    "application/zip"
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

fn make_result(mime: &str) -> DetectionResult {
    DetectionResult::with_mime(None, 1.0, None, Some(mime.to_string()))
}

/// Check `data` for known binary file magic numbers.
pub fn detect_magic(data: &[u8]) -> Option<DetectionResult> {
    if data.is_empty() {
        return None;
    }

    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        let box_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if box_size >= 8 && box_size <= data.len() {
            let brand = &data[8..12];
            if brand == b"avif" || brand == b"avis" {
                return Some(make_result("image/avif"));
            }
            if brand == b"heic" || brand == b"heix" {
                return Some(make_result("image/heic"));
            }
            if brand == b"mif1" || brand == b"msf1" {
                return Some(make_result("image/heif"));
            }
            if brand == b"M4A " || brand == b"M4B " || brand == b"F4A " {
                return Some(make_result("audio/mp4"));
            }
            if brand == b"qt  " {
                return Some(make_result("video/quicktime"));
            }
            return Some(make_result("video/mp4"));
        }
    }

    if data.starts_with(b"RIFF") && data.len() >= 12 {
        if let Some(mime) = riff_subtype(&data[8..12]) {
            return Some(make_result(mime));
        }
    }

    if data.starts_with(b"FORM") && data.len() >= 12 {
        if let Some(mime) = form_subtype(&data[8..12]) {
            return Some(make_result(mime));
        }
    }

    if data.starts_with(ZIP_SIGNATURE) {
        return Some(make_result(classify_zip(data)));
    }

    if data.len() >= 8 && &data[..4] == b"\xca\xfe\xba\xbe" {
        let nfat = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        if nfat <= 20 {
            return Some(make_result("application/x-mach-binary"));
        }
        return Some(make_result("application/java-vm"));
    }

    for (prefix, mime) in MAGIC_NUMBERS {
        if data.starts_with(prefix) {
            return Some(make_result(mime));
        }
    }

    if data.len() >= TAR_OFFSET + 6 {
        let sig = &data[TAR_OFFSET..TAR_OFFSET + 6];
        if TAR_SIGNATURES.contains(&sig) {
            return Some(make_result("application/x-tar"));
        }
    }

    None
}
