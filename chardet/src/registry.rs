//! Encoding registry and name lookup, mirroring `chardet.registry`.

use crate::enums::Era;
use once_cell::sync::Lazy;
use std::collections::HashMap;

pub use crate::registry_data::{EncodingInfo, PY_ALIASES, REGISTRY_ENTRIES};

/// Normalize an encoding name the way CPython's codec lookup does:
/// lowercase and replace every non-alphanumeric byte with `_`.
pub fn normalize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

static NAME_INDEX: Lazy<HashMap<String, usize>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for (i, e) in REGISTRY_ENTRIES.iter().enumerate() {
        m.entry(normalize_name(e.name)).or_insert(i);
        for alias in e.aliases {
            m.entry(normalize_name(alias)).or_insert(i);
        }
    }
    m
});

/// Normalized CPython alias -> canonical registry index.
static PY_ALIAS_INDEX: Lazy<HashMap<&'static str, usize>> = Lazy::new(|| {
    let mut m = HashMap::new();
    for &(alias, canonical) in PY_ALIASES {
        if let Some(&i) = NAME_INDEX.get(normalize_name(canonical).as_str()) {
            m.entry(alias).or_insert(i);
        } else if let Some(&i) = NAME_INDEX.get(alias) {
            m.entry(alias).or_insert(i);
        }
    }
    m
});

/// Extra names CPython resolves that the registry's alias lists do not spell out.
static EXTRA_NAMES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("chinese", "gb18030");
    m.insert("korean", "euc_kr");
    // Python codec aliases used by chardet's compat display names.
    m.insert("ibm855", "cp855");
    m.insert("ibm866", "cp866");
    m
});

/// Canonical registry entry for `name`, if known.
pub fn lookup(name: &str) -> Option<&'static EncodingInfo> {
    let norm = normalize_name(name);
    if let Some(&i) = NAME_INDEX.get(&norm) {
        return Some(&REGISTRY_ENTRIES[i]);
    }
    if let Some(&i) = PY_ALIAS_INDEX.get(norm.as_str()) {
        return Some(&REGISTRY_ENTRIES[i]);
    }
    if let Some(canon) = EXTRA_NAMES.get(norm.as_str()) {
        return lookup(canon);
    }
    None
}

/// Canonical encoding name for `name`, or `None` if unknown.
pub fn lookup_encoding(name: &str) -> Option<&'static str> {
    lookup(name).map(|e| e.name)
}

/// Validate and normalize a single encoding name, mirroring `_validate_encoding`.
pub fn validate_encoding(name: &str) -> Result<&'static str, String> {
    lookup_encoding(name).ok_or_else(|| format!("Unknown encoding {name:?}"))
}

/// Normalize an optional collection of encoding names.
pub fn normalize_encodings(
    encodings: Option<&[String]>,
) -> Result<Option<Vec<&'static str>>, String> {
    match encodings {
        None => Ok(None),
        Some(list) => {
            let mut out = Vec::with_capacity(list.len());
            for name in list {
                out.push(validate_encoding(name)?);
            }
            if out.is_empty() {
                return Err("encoding filter must not be empty".to_string());
            }
            Ok(Some(out))
        }
    }
}

/// Return registry entries matching the given era / include / exclude filters.
///
/// Mirrors `chardet.registry.get_candidates`.
pub fn get_candidates(
    era: u8,
    include_encodings: Option<&[String]>,
    exclude_encodings: Option<&[String]>,
) -> Vec<&'static EncodingInfo> {
    REGISTRY_ENTRIES
        .iter()
        .filter(|enc| enc.era & era != 0)
        .filter(|enc| match include_encodings {
            Some(inc) => inc.iter().any(|s| s == enc.name),
            None => true,
        })
        .filter(|enc| match exclude_encodings {
            Some(exc) => !exc.iter().any(|s| s == enc.name),
            None => true,
        })
        .collect()
}

/// All registry entries, in registry order.
pub fn all() -> &'static [EncodingInfo] {
    REGISTRY_ENTRIES
}

/// Whether `enc` is marked as a multibyte encoding.
pub fn is_multibyte(name: &str) -> bool {
    lookup(name).map(|e| e.is_multibyte).unwrap_or(false)
}

/// The era of an encoding, or `Era::ALL` if unknown.
pub fn era_of(name: &str) -> u8 {
    lookup(name).map(|e| e.era).unwrap_or(Era::ALL)
}
