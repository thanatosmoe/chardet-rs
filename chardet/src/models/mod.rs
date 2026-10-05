//! Bigram model loading and scoring, mirroring `chardet.models`.

pub mod format;

use crate::registry::REGISTRY_ENTRIES;
use format::{parse_models_bin, parse_rowmax_bin, rowmax_from_table};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

static MODELS_BIN: &[u8] = include_bytes!("../../models/models.bin");
static IDF_BIN: &[u8] = include_bytes!("../../models/idf.bin");
static ROWMAX_BIN: &[u8] = include_bytes!("../../models/rowmax.bin");

/// Pseudo-language for models trained on data with no linguistic content.
pub const ART_LANGUAGE: &str = "zxx";

/// ADR-0005's rare-language set, shared with the postprocess arbitration gate.
pub static RARE_LANGUAGES: &[&str] = &["gd", "cy", "ga", "br"];

/// Thin-input gate for demoting rare-language winners.
pub const THIN_RARE_MAX_BYTES: usize = 128;
/// Maximum lead for a rare win on a short input to count as noise.
pub const THIN_RARE_MARGIN: f64 = 0.03;

/// Byte values whose repeated-run bigrams are collapsed out of the profile.
fn is_ascii_whitespace(b: u8) -> bool {
    matches!(b, 0x20 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0xA0)
}

/// One language variant of an encoding's model.
pub struct ModelVariant {
    pub lang: Option<String>,
    pub table: Vec<u8>,
    pub key: String,
}

pub struct ModelData {
    entries: Vec<ModelVariant>,
    index: HashMap<String, Vec<usize>>,
    norms: HashMap<String, f64>,
    rowmax: HashMap<String, Vec<u8>>,
    idf: Vec<u8>,
    single_lang: HashMap<String, String>,
}

impl ModelData {
    pub fn variants(&self, encoding: &str) -> Option<&[usize]> {
        self.index.get(encoding).map(|v| v.as_slice())
    }

    pub fn variant(&self, idx: usize) -> &ModelVariant {
        &self.entries[idx]
    }

    pub fn norm(&self, key: &str) -> Option<f64> {
        self.norms.get(key).copied()
    }

    pub fn idf(&self) -> &[u8] {
        &self.idf
    }

    pub fn rowmax(&self, key: &str) -> Option<&[u8]> {
        self.rowmax.get(key).map(|v| v.as_slice())
    }

    pub fn has_variants(&self, encoding: &str) -> bool {
        self.index.contains_key(encoding)
    }

    pub fn infer_language(&self, encoding: &str) -> Option<&str> {
        self.single_lang.get(encoding).map(|s| s.as_str())
    }
}

static MODEL_DATA: Lazy<ModelData> = Lazy::new(build_model_data);

/// Access the lazily-loaded model data.
pub fn model_data() -> &'static ModelData {
    &MODEL_DATA
}

fn build_model_data() -> ModelData {
    let parsed = parse_models_bin(MODELS_BIN).expect("bundled models.bin must parse");
    let format::ModelsBin {
        names,
        tables,
        norms,
    } = parsed;

    // Build index keyed by model name with canonical alias resolution.
    let mut entries: Vec<ModelVariant> = Vec::with_capacity(names.len());
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut tables = tables.into_iter();
    for (i, name) in names.iter().enumerate() {
        let table = tables.next().expect("table count matches names");
        let (lang, enc) = name.split_once('/').unwrap_or(("", name.as_str()));
        let lang_opt = if lang.is_empty() || lang == "None" {
            None
        } else {
            Some(lang.to_string())
        };
        entries.push(ModelVariant {
            lang: lang_opt,
            table,
            key: name.clone(),
        });
        index.entry(enc.to_string()).or_default().push(i);
    }
    // Resolve aliases: copy an entry under its canonical name if absent.
    let keys: Vec<String> = index.keys().cloned().collect();
    for enc_name in keys {
        if let Some(canonical) = crate::registry::lookup_encoding(&enc_name) {
            if !index.contains_key(canonical) {
                let v = index.get(&enc_name).cloned().unwrap();
                index.insert(canonical.to_string(), v);
            }
        }
    }

    // Row maxima, validated against the models.bin digest.
    let digest: [u8; 32] = Sha256::digest(MODELS_BIN).into();
    let rowmax_tables = parse_rowmax_bin(ROWMAX_BIN, &digest, &names);
    let mut rowmax = HashMap::new();
    match rowmax_tables {
        Some(tables) => {
            for (i, name) in names.iter().enumerate() {
                rowmax.insert(name.clone(), tables[i].clone());
            }
        }
        None => {
            for (i, name) in names.iter().enumerate() {
                rowmax.insert(name.clone(), rowmax_from_table(&entries[i].table));
            }
        }
    }

    let idf = if IDF_BIN.len() == 65536 {
        IDF_BIN.to_vec()
    } else {
        vec![1u8; 65536]
    };

    // Single-language encodings (registry languages len == 1).
    let mut single_lang = HashMap::new();
    for e in REGISTRY_ENTRIES {
        if e.languages.len() == 1 {
            single_lang.insert(e.name.to_string(), e.languages[0].to_string());
        }
    }

    ModelData {
        entries,
        index,
        norms,
        rowmax,
        idf,
        single_lang,
    }
}

/// Pre-computed IDF-weighted bigram distribution for a data sample.
pub struct BigramProfile {
    pub nonzero: Vec<u32>,
    pub values: Vec<u64>,
    pub row_freq: Vec<u64>,
    pub nonzero_rows: Vec<u16>,
    pub input_norm: f64,
    pub weight_sum: u64,
}

impl BigramProfile {
    pub fn new(data: &[u8]) -> Self {
        if data.len() < 2 {
            return Self::empty();
        }
        let idf = data_idf();
        let mut freq = vec![0u64; 65536];
        let mut nonzero: Vec<u32> = Vec::new();
        let mut w_sum: u64 = 0;
        for i in 0..data.len() - 1 {
            let b1 = data[i];
            let b2 = data[i + 1];
            if b1 == b2 && is_ascii_whitespace(b1) {
                continue;
            }
            let idx = ((b1 as u32) << 8) | b2 as u32;
            let w = idf[idx as usize] as u64;
            if freq[idx as usize] == 0 {
                nonzero.push(idx);
            }
            freq[idx as usize] += w;
            w_sum += w;
        }
        let values: Vec<u64> = nonzero.iter().map(|&i| freq[i as usize]).collect();
        Self::finish(nonzero, values, w_sum)
    }

    /// Build from pre-computed weighted frequencies (insertion order preserved).
    pub fn from_weighted_freq(weighted: &[(u32, u64)]) -> Self {
        let mut nonzero = Vec::new();
        let mut values = Vec::new();
        let mut w_sum = 0u64;
        for &(idx, count) in weighted {
            if count != 0 {
                nonzero.push(idx);
                values.push(count);
                w_sum += count;
            }
        }
        Self::finish(nonzero, values, w_sum)
    }

    fn empty() -> Self {
        BigramProfile {
            nonzero: Vec::new(),
            values: Vec::new(),
            row_freq: vec![0; 256],
            nonzero_rows: Vec::new(),
            input_norm: 0.0,
            weight_sum: 0,
        }
    }

    fn finish(nonzero: Vec<u32>, values: Vec<u64>, weight_sum: u64) -> Self {
        let mut norm_sq: u128 = 0;
        let mut row_freq = vec![0u64; 256];
        for (i, &idx) in nonzero.iter().enumerate() {
            let v = values[i];
            norm_sq += (v as u128) * (v as u128);
            row_freq[(idx >> 8) as usize] += v;
        }
        let input_norm = (norm_sq as f64).sqrt();
        let nonzero_rows: Vec<u16> = (0..256u16).filter(|&b| row_freq[b as usize] != 0).collect();
        BigramProfile {
            nonzero,
            values,
            row_freq,
            nonzero_rows,
            input_norm,
            weight_sum,
        }
    }
}

fn data_idf() -> &'static [u8] {
    model_data().idf()
}

/// Score a profile against one model table (cosine similarity).
pub fn score_with_profile(profile: &BigramProfile, table: &[u8], model_key: &str) -> f64 {
    if profile.input_norm == 0.0 {
        return 0.0;
    }
    let model_norm = match model_data().norm(model_key) {
        Some(n) => n,
        None => {
            let mut sq: u128 = 0;
            for &v in table.iter() {
                sq += (v as u128) * (v as u128);
            }
            (sq as f64).sqrt()
        }
    };
    if model_norm == 0.0 {
        return 0.0;
    }
    let mut dot: u128 = 0;
    for (i, &idx) in profile.nonzero.iter().enumerate() {
        dot += (table[idx as usize] as u128) * (profile.values[i] as u128);
    }
    (dot as f64) / (model_norm * profile.input_norm)
}

/// Score data against all language variants of an encoding.
///
/// Returns `(best_score, best_language)`.
pub fn score_best_language(
    data: &[u8],
    encoding: &str,
    profile: Option<&BigramProfile>,
    demote_thin_rare: bool,
) -> (f64, Option<String>) {
    if data.is_empty() && profile.is_none() {
        return (0.0, None);
    }
    let md = model_data();
    let variants = match md.variants(encoding) {
        Some(v) if !v.is_empty() => v,
        _ => return (0.0, None),
    };
    let owned_profile;
    let profile = match profile {
        Some(p) => p,
        None => {
            owned_profile = BigramProfile::new(data);
            &owned_profile
        }
    };

    let mut best_score = 0.0f64;
    let mut best_lang: Option<String> = None;
    let mut best_prevalent = 0.0f64;
    let mut best_prevalent_lang: Option<String> = None;
    for &vi in variants {
        let v = md.variant(vi);
        let s = score_with_profile(profile, &v.table, &v.key);
        if s > best_score {
            best_score = s;
            best_lang = v.lang.clone();
        }
        if demote_thin_rare {
            if let Some(lang) = &v.lang {
                if !RARE_LANGUAGES.contains(&lang.as_str())
                    && lang != ART_LANGUAGE
                    && s > best_prevalent
                {
                    best_prevalent = s;
                    best_prevalent_lang = Some(lang.clone());
                }
            }
        }
    }

    if demote_thin_rare {
        if let (Some(bl), Some(bp)) = (&best_lang, &best_prevalent_lang) {
            if RARE_LANGUAGES.contains(&bl.as_str())
                && best_score - best_prevalent < THIN_RARE_MARGIN
            {
                best_lang = Some(bp.clone());
            }
        }
    }

    (best_score, best_lang)
}
