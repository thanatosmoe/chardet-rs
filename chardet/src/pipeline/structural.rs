//! Stage 2b: Multi-byte structural probing, mirroring `chardet.pipeline.structural`.

use crate::registry::EncodingInfo;
use crate::result::PipelineContext;

type Analysis = (f64, usize, usize);

fn analyze_shift_jis(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0x81..=0x9F).contains(&b) || (0xE0..=0xEF).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() {
                let trail = data[i + 1];
                if (0x40..=0x7E).contains(&trail) || (0x80..=0xFC).contains(&trail) {
                    valid_count += 1;
                    leads[b as usize] = true;
                    mb += 1;
                    if trail > 0x7F {
                        mb += 1;
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyze_cp932(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0x81..=0x9F).contains(&b) || (0xE0..=0xFC).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() {
                let trail = data[i + 1];
                if (0x40..=0x7E).contains(&trail) || (0x80..=0xFC).contains(&trail) {
                    valid_count += 1;
                    leads[b as usize] = true;
                    mb += 1;
                    if trail > 0x7F {
                        mb += 1;
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyze_euc_jp(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == 0x8E {
            lead_count += 1;
            if i + 1 < data.len() && (0xA1..=0xDF).contains(&data[i + 1]) {
                valid_count += 1;
                leads[b as usize] = true;
                mb += 2;
                i += 2;
                continue;
            }
            i += 1;
        } else if b == 0x8F {
            lead_count += 1;
            if i + 2 < data.len()
                && (0xA1..=0xFE).contains(&data[i + 1])
                && (0xA1..=0xFE).contains(&data[i + 2])
            {
                valid_count += 1;
                leads[b as usize] = true;
                mb += 3;
                i += 3;
                continue;
            }
            i += 1;
        } else if (0xA1..=0xFE).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() && (0xA1..=0xFE).contains(&data[i + 1]) {
                valid_count += 1;
                leads[b as usize] = true;
                mb += 2;
                i += 2;
                continue;
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyze_euc_kr(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0xA1..=0xFE).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() && (0xA1..=0xFE).contains(&data[i + 1]) {
                valid_count += 1;
                leads[b as usize] = true;
                mb += 2;
                i += 2;
                continue;
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyze_cp949(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0x81..=0xC8).contains(&b) || (0xCA..=0xFD).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() {
                let trail = data[i + 1];
                if (0x41..=0x5A).contains(&trail)
                    || (0x61..=0x7A).contains(&trail)
                    || (0x81..=0xFE).contains(&trail)
                {
                    valid_count += 1;
                    leads[b as usize] = true;
                    mb += 1;
                    if trail > 0x7F {
                        mb += 1;
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyze_gb18030(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0x81..=0xFE).contains(&b) {
            lead_count += 1;
            if i + 3 < data.len()
                && (0x30..=0x39).contains(&data[i + 1])
                && (0x81..=0xFE).contains(&data[i + 2])
                && (0x30..=0x39).contains(&data[i + 3])
            {
                valid_count += 1;
                leads[b as usize] = true;
                mb += 2;
                i += 4;
                continue;
            }
            if (0xA1..=0xF7).contains(&b)
                && i + 1 < data.len()
                && (0xA1..=0xFE).contains(&data[i + 1])
            {
                valid_count += 1;
                leads[b as usize] = true;
                mb += 2;
                i += 2;
                continue;
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

#[allow(dead_code)]
fn analyze_big5(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0xA1..=0xF9).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() {
                let trail = data[i + 1];
                if (0x40..=0x7E).contains(&trail) || (0xA1..=0xFE).contains(&trail) {
                    valid_count += 1;
                    leads[b as usize] = true;
                    mb += 1;
                    if trail > 0x7F {
                        mb += 1;
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyze_big5hkscs(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0x87..=0xFE).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() {
                let trail = data[i + 1];
                if (0x40..=0x7E).contains(&trail) || (0xA1..=0xFE).contains(&trail) {
                    valid_count += 1;
                    leads[b as usize] = true;
                    mb += 1;
                    if trail > 0x7F {
                        mb += 1;
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyze_johab(data: &[u8]) -> Analysis {
    let mut lead_count = 0usize;
    let mut valid_count = 0usize;
    let mut mb = 0usize;
    let mut leads = [false; 256];
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if (0x84..=0xD3).contains(&b) || (0xD8..=0xDE).contains(&b) || (0xE0..=0xF9).contains(&b) {
            lead_count += 1;
            if i + 1 < data.len() {
                let trail = data[i + 1];
                if (0x31..=0x7E).contains(&trail) || (0x91..=0xFE).contains(&trail) {
                    valid_count += 1;
                    leads[b as usize] = true;
                    if b > 0x7F {
                        mb += 1;
                    }
                    if trail > 0x7F {
                        mb += 1;
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    let ratio = if lead_count > 0 {
        valid_count as f64 / lead_count as f64
    } else {
        0.0
    };
    (ratio, mb, leads.iter().filter(|&&x| x).count())
}

fn analyzer_for(name: &str) -> Option<fn(&[u8]) -> Analysis> {
    Some(match name {
        "shift_jis_2004" => analyze_shift_jis,
        "cp932" => analyze_cp932,
        "euc_jis_2004" => analyze_euc_jp,
        "euc_kr" => analyze_euc_kr,
        "cp949" => analyze_cp949,
        "gb18030" => analyze_gb18030,
        "big5hkscs" => analyze_big5hkscs,
        "johab" => analyze_johab,
        _ => return None,
    })
}

fn get_analysis(data: &[u8], name: &'static str, ctx: &mut PipelineContext) -> Option<Analysis> {
    if let Some(&cached) = ctx.analysis_cache.get(name) {
        return Some(cached);
    }
    let f = analyzer_for(name)?;
    let result = f(data);
    ctx.analysis_cache.insert(name, result);
    Some(result)
}

/// 0.0-1.0 fit of `data` to the encoding's multi-byte structure.
pub fn compute_structural_score(data: &[u8], enc: &EncodingInfo, ctx: &mut PipelineContext) -> f64 {
    if data.is_empty() || !enc.is_multibyte {
        return 0.0;
    }
    match get_analysis(data, enc.name, ctx) {
        Some((ratio, _, _)) => ratio,
        None => 0.0,
    }
}

/// Ratio of non-ASCII bytes that participate in valid multi-byte sequences.
pub fn compute_multibyte_byte_coverage(
    data: &[u8],
    enc: &EncodingInfo,
    ctx: &mut PipelineContext,
    non_ascii_count: Option<usize>,
) -> f64 {
    if data.is_empty() || !enc.is_multibyte {
        return 0.0;
    }
    let analysis = match get_analysis(data, enc.name, ctx) {
        Some(a) => a,
        None => return 0.0,
    };
    let mb_bytes = analysis.1;
    let non_ascii = match non_ascii_count {
        Some(c) => c,
        None => data.iter().filter(|&&b| b >= 0x80).count(),
    };
    if non_ascii == 0 {
        return 0.0;
    }
    mb_bytes as f64 / non_ascii as f64
}

/// Count distinct lead byte values in valid multi-byte pairs.
pub fn compute_lead_byte_diversity(
    data: &[u8],
    enc: &EncodingInfo,
    ctx: &mut PipelineContext,
) -> usize {
    if data.is_empty() || !enc.is_multibyte {
        return 0;
    }
    match get_analysis(data, enc.name, ctx) {
        Some(a) => a.2,
        None => 256,
    }
}
