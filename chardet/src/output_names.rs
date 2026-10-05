//! Public-API encoding-name remapping, mirroring `chardet.output_names`.

use crate::codecs::decodes_without_error;
use crate::result::DetectionResult;
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Preferred superset name for each encoding (`prefer_superset` option).
pub static PREFERRED_SUPERSET: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("ascii", "cp1252");
    m.insert("euc_kr", "cp949");
    m.insert("iso8859-1", "cp1252");
    m.insert("iso8859-2", "cp1250");
    m.insert("iso8859-5", "cp1251");
    m.insert("iso8859-6", "cp1256");
    m.insert("iso8859-7", "cp1253");
    m.insert("iso8859-8", "cp1255");
    m.insert("iso8859-9", "cp1254");
    m.insert("iso8859-11", "cp874");
    m.insert("iso8859-13", "cp1257");
    m.insert("tis-620", "cp874");
    m
});

/// Python codec name -> chardet 5.x/6.x compatible display name.
pub static COMPAT_NAMES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for (k, v) in [
        ("big5hkscs", "Big5"),
        ("cp855", "IBM855"),
        ("cp866", "IBM866"),
        ("cp874", "CP874"),
        ("cp932", "CP932"),
        ("cp949", "CP949"),
        ("euc_jis_2004", "EUC-JP"),
        ("euc_kr", "EUC-KR"),
        ("gb18030", "GB18030"),
        ("hz", "HZ-GB-2312"),
        ("iso2022_jp_2", "ISO-2022-JP"),
        ("iso2022_kr", "ISO-2022-KR"),
        ("iso8859-1", "ISO-8859-1"),
        ("iso8859-2", "ISO-8859-2"),
        ("iso8859-5", "ISO-8859-5"),
        ("iso8859-6", "ISO-8859-6"),
        ("iso8859-7", "ISO-8859-7"),
        ("iso8859-8", "ISO-8859-8"),
        ("iso8859-9", "ISO-8859-9"),
        ("iso8859-13", "ISO-8859-13"),
        ("johab", "Johab"),
        ("koi8-r", "KOI8-R"),
        ("mac-cyrillic", "MacCyrillic"),
        ("mac-roman", "MacRoman"),
        ("shift_jis_2004", "SHIFT_JIS"),
        ("tis-620", "TIS-620"),
        ("utf-16", "UTF-16"),
        ("utf-32", "UTF-32"),
        ("utf-8-sig", "UTF-8-SIG"),
        ("cp1250", "Windows-1250"),
        ("cp1251", "Windows-1251"),
        ("cp1252", "Windows-1252"),
        ("cp1253", "Windows-1253"),
        ("cp1254", "Windows-1254"),
        ("cp1255", "Windows-1255"),
        ("cp1256", "Windows-1256"),
        ("cp1257", "Windows-1257"),
        ("kz1048", "KZ1048"),
        ("mac-greek", "MacGreek"),
        ("mac-iceland", "MacIceland"),
        ("mac-latin2", "MacLatin2"),
        ("mac-turkish", "MacTurkish"),
    ] {
        m.insert(k, v);
    }
    m
});

fn remap(result: &mut DetectionResult, mapping: &HashMap<&'static str, &'static str>) {
    if let Some(enc) = result.encoding.as_deref() {
        if let Some(new) = mapping.get(enc) {
            result.encoding = Some((*new).to_string());
        }
    }
}

/// Replace the encoding name with its preferred Windows/CP superset.
pub fn apply_preferred_superset(result: &mut DetectionResult, data: Option<&[u8]>) {
    let enc = match result.encoding.as_deref() {
        Some(e) => e,
        None => return,
    };
    let superset = match PREFERRED_SUPERSET.get(enc) {
        Some(s) => *s,
        None => return,
    };
    if data.is_none() || decodes_without_error(data.unwrap(), superset) {
        result.encoding = Some(superset.to_string());
    }
}

/// Convert internal codec names to chardet 5.x/6.x compatible names.
pub fn apply_compat_names(result: &mut DetectionResult) {
    remap(result, &COMPAT_NAMES);
}
