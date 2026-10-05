//! Model-artifact parsing (`models.bin`, `rowmax.bin`), mirroring
//! `chardet.models._format`.

use flate2::read::ZlibDecoder;
use std::collections::HashMap;
use std::io::Read;

pub const MODELS_MAGIC: &[u8; 4] = b"CMD2";
pub const ROWMAX_MAGIC: &[u8; 4] = b"CRM1";
pub const ROWMAX_HEADER_SIZE: usize = 4 + 32;

/// Parsed `models.bin`: ordered `(name, table)` pairs plus L2 norms.
pub struct ModelsBin {
    pub names: Vec<String>,
    pub tables: Vec<Vec<u8>>,
    pub norms: HashMap<String, f64>,
}

fn be_u32(data: &[u8], off: usize) -> Option<u32> {
    data.get(off..off + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn be_f64(data: &[u8], off: usize) -> Option<f64> {
    data.get(off..off + 8)
        .map(|s| f64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

/// Parse the v2 dense zlib-compressed `models.bin` format.
pub fn parse_models_bin(data: &[u8]) -> Result<ModelsBin, String> {
    if data.len() < 8 || &data[0..4] != MODELS_MAGIC {
        return Err("corrupt models.bin: missing CMD2 magic".into());
    }
    let num_models = be_u32(data, 4).ok_or("corrupt models.bin: truncated")? as usize;
    if num_models > 10_000 {
        return Err(format!(
            "corrupt models.bin: num_models={num_models} exceeds limit"
        ));
    }
    let mut off = 8;
    let mut names = Vec::with_capacity(num_models);
    let mut norms = HashMap::with_capacity(num_models);
    for _ in 0..num_models {
        let name_len = be_u32(data, off).ok_or("corrupt models.bin: truncated")? as usize;
        off += 4;
        if name_len > 256 {
            return Err(format!(
                "corrupt models.bin: name_len={name_len} exceeds 256"
            ));
        }
        let raw = data
            .get(off..off + name_len)
            .ok_or("corrupt models.bin: truncated name")?;
        let name = std::str::from_utf8(raw)
            .map_err(|e| format!("corrupt models.bin: {e}"))?
            .to_string();
        off += name_len;
        let norm = be_f64(data, off).ok_or("corrupt models.bin: truncated norm")?;
        off += 8;
        names.push(name.clone());
        norms.insert(name, norm);
    }

    let mut decoder = ZlibDecoder::new(&data[off..]);
    let mut blob = Vec::new();
    decoder
        .read_to_end(&mut blob)
        .map_err(|e| format!("corrupt models.bin: {e}"))?;
    let expected = num_models * 65536;
    if blob.len() != expected {
        return Err(format!(
            "corrupt models.bin: decompressed size {} != expected {}",
            blob.len(),
            expected
        ));
    }
    let tables: Vec<Vec<u8>> = blob.chunks_exact(65536).map(|c| c.to_vec()).collect();
    Ok(ModelsBin {
        names,
        tables,
        norms,
    })
}

/// Parse `rowmax.bin`, validating magic, digest, and size.
pub fn parse_rowmax_bin(
    data: &[u8],
    models_digest: &[u8; 32],
    model_keys: &[String],
) -> Option<Vec<Vec<u8>>> {
    if data.len() < ROWMAX_HEADER_SIZE
        || &data[0..4] != ROWMAX_MAGIC
        || &data[4..ROWMAX_HEADER_SIZE] != models_digest
        || data.len() != ROWMAX_HEADER_SIZE + model_keys.len() * 256
    {
        return None;
    }
    let mut out = Vec::with_capacity(model_keys.len());
    for i in 0..model_keys.len() {
        let start = ROWMAX_HEADER_SIZE + i * 256;
        out.push(data[start..start + 256].to_vec());
    }
    Some(out)
}

/// Derive a model's 256-byte row-maxima table from its dense table.
pub fn rowmax_from_table(table: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    for start in (0..65536).step_by(256) {
        out.push(*table[start..start + 256].iter().max().unwrap());
    }
    out
}
