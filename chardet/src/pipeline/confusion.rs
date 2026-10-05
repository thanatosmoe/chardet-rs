//! Confusion group resolution for similar single-byte encodings.

use crate::codecs::{category_of_cp, decode_byte, letter_case_table};
use crate::models::{model_data, score_with_profile, BigramProfile, ART_LANGUAGE};
use crate::registry::lookup_encoding;
use crate::result::DetectionResult;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};

type Categories = HashMap<u8, (String, String)>;
type DistMaps = HashMap<(String, String), (HashSet<u8>, Categories)>;

static CONFUSION_BIN: &[u8] = include_bytes!("../../models/confusion.bin");

fn int_to_category(i: u8) -> &'static str {
    crate::codecs::categories_data::CATEGORY_NAMES
        .get(i as usize)
        .copied()
        .unwrap_or("Cn")
}

pub fn load_confusion_data() -> &'static DistMaps {
    static DATA: Lazy<DistMaps> = Lazy::new(build_confusion_data);
    &DATA
}

fn build_confusion_data() -> DistMaps {
    let data = CONFUSION_BIN;
    let mut result = DistMaps::new();
    if data.len() < 2 {
        return result;
    }
    let mut off = 0usize;
    let num_pairs = ((data[off] as usize) << 8) | data[off + 1] as usize;
    off += 2;
    for _ in 0..num_pairs {
        if off >= data.len() {
            break;
        }
        let la = data[off] as usize;
        off += 1;
        let name_a = String::from_utf8_lossy(&data[off..off + la]).into_owned();
        off += la;
        let lb = data[off] as usize;
        off += 1;
        let name_b = String::from_utf8_lossy(&data[off..off + lb]).into_owned();
        off += lb;
        let num_diffs = data[off] as usize;
        off += 1;
        let mut diff = HashSet::new();
        let mut cats = Categories::new();
        for _ in 0..num_diffs {
            let bv = data[off];
            let ca = data[off + 1];
            let cb = data[off + 2];
            off += 3;
            diff.insert(bv);
            cats.insert(
                bv,
                (
                    int_to_category(ca).to_string(),
                    int_to_category(cb).to_string(),
                ),
            );
        }
        let na = lookup_encoding(&name_a)
            .map(|s| s.to_string())
            .unwrap_or(name_a);
        let nb = lookup_encoding(&name_b)
            .map(|s| s.to_string())
            .unwrap_or(name_b);
        result.insert((na, nb), (diff, cats));
    }
    result
}

// Unicode general category preference scores for voting resolution.
fn category_preference(cat: &str) -> i32 {
    match cat {
        "Lu" | "Ll" | "Lt" => 10,
        "Lm" | "Lo" => 9,
        "Nd" => 8,
        "Nl" | "No" => 7,
        "Pc" | "Pd" | "Ps" | "Pe" | "Pi" | "Pf" | "Po" => 6,
        "Sc" | "Sm" => 5,
        "Sk" | "So" => 4,
        "Zs" | "Zl" | "Zp" => 3,
        "Cf" => 2,
        "Cc" | "Co" => 1,
        "Cs" | "Cn" => 0,
        "Mn" | "Mc" | "Me" => 5,
        _ => 0,
    }
}

const IMPLAUSIBLE_LETTER_PREFERENCE: i32 = 2;
const DECISIVE_VOTE_MARGIN: i32 = 8;
const DECISIVE_MIN_EVENTS: i32 = 2;
const MAX_VOTE_OCCURRENCES: usize = 256;
const DENSE_HIT_DIVISOR: usize = 4;

fn zero_case_table() -> &'static [u8; 256] {
    static ZERO: [u8; 256] = [0u8; 256];
    &ZERO
}

fn case_table(enc: &str) -> &'static [u8; 256] {
    letter_case_table(enc).unwrap_or_else(zero_case_table)
}

fn context_preference(cat: &str, left: u8, right: u8, table: &[u8; 256]) -> i32 {
    let pref = category_preference(cat);
    if !cat.starts_with('L') {
        return pref;
    }
    let left_kind = table[left as usize];
    let right_kind = table[right as usize];
    if left_kind == 0 && right_kind == 0 {
        return IMPLAUSIBLE_LETTER_PREFERENCE;
    }
    if cat == "Ll" && right_kind == 1 {
        return IMPLAUSIBLE_LETTER_PREFERENCE;
    }
    pref
}

fn pair_byte_tables(diff: &HashSet<u8>) -> (Vec<u8>, [bool; 256]) {
    let mut member = [false; 256];
    for &b in diff {
        member[b as usize] = true;
    }
    let non_diff: Vec<u8> = (0..=255u8).filter(|&b| !member[b as usize]).collect();
    (non_diff, member)
}

fn vote_with_margin(
    data: &[u8],
    enc_a: &str,
    enc_b: &str,
    diff: &HashSet<u8>,
    categories: &Categories,
) -> (Option<String>, i32, i32, i32) {
    let mut relevant: Vec<u8> = Vec::new();
    let mut seen = [false; 256];
    for &b in data {
        if diff.contains(&b) && !seen[b as usize] {
            seen[b as usize] = true;
            relevant.push(b);
        }
    }
    if relevant.is_empty() {
        return (None, 0, 0, 0);
    }
    let table_a = case_table(enc_a);
    let table_b = case_table(enc_b);
    let mut votes_a = 0i32;
    let mut votes_b = 0i32;
    let mut demotion_a = 0i32;
    let mut demotion_b = 0i32;
    let mut events_a = 0i32;
    let mut events_b = 0i32;
    let end = data.len().saturating_sub(1);
    for &bv in &relevant {
        let (cat_a, cat_b) = match categories.get(&bv) {
            Some(c) => c,
            None => continue,
        };
        let mut examined = 0usize;
        let mut pos = 0usize;
        // find occurrences of bv
        while pos < data.len() && examined < MAX_VOTE_OCCURRENCES {
            match data[pos..].iter().position(|&x| x == bv) {
                Some(p) => pos += p,
                None => break,
            }
            let left = if pos > 0 { data[pos - 1] } else { 0 };
            let right = if pos < end { data[pos + 1] } else { 0 };
            let pref_a = context_preference(cat_a, left, right, table_a);
            let pref_b = context_preference(cat_b, left, right, table_b);
            if pref_a > pref_b {
                if !(cat_a.starts_with('L') && cat_b.starts_with('P')) {
                    votes_a += pref_a - pref_b;
                    if cat_b.starts_with('L') && pref_b == IMPLAUSIBLE_LETTER_PREFERENCE {
                        demotion_a += pref_a - pref_b;
                        events_a += 1;
                    }
                }
            } else if pref_b > pref_a && !(cat_b.starts_with('L') && cat_a.starts_with('P')) {
                votes_b += pref_b - pref_a;
                if cat_a.starts_with('L') && pref_a == IMPLAUSIBLE_LETTER_PREFERENCE {
                    demotion_b += pref_b - pref_a;
                    events_b += 1;
                }
            }
            examined += 1;
            pos += 1;
        }
    }
    if votes_a > votes_b {
        return (Some(enc_a.to_string()), votes_a - votes_b, demotion_a, events_a);
    }
    if votes_b > votes_a {
        return (Some(enc_b.to_string()), votes_b - votes_a, demotion_b, events_b);
    }
    (None, 0, 0, 0)
}

fn modelled_languages(enc: &str) -> HashSet<String> {
    let md = model_data();
    let mut out = HashSet::new();
    if let Some(v) = md.variants(enc) {
        for &i in v {
            if let Some(l) = &md.variant(i).lang {
                out.insert(l.clone());
            }
        }
    }
    out
}

fn comparable_languages(
    enc_a: &str,
    enc_b: &str,
    languages: &HashSet<String>,
) -> Option<HashSet<String>> {
    if languages.is_empty() {
        return None;
    }
    let langs_a = modelled_languages(enc_a);
    let langs_b = modelled_languages(enc_b);
    let shared: HashSet<String> = langs_a.intersection(&langs_b).cloned().collect();
    if !languages.is_subset(&shared) {
        return None;
    }
    if langs_a.contains(ART_LANGUAGE) || langs_b.contains(ART_LANGUAGE) {
        let mut out = shared;
        out.insert(ART_LANGUAGE.to_string());
        return Some(out);
    }
    Some(shared)
}

/// Public wrapper around the comparable-languages restriction.
pub fn comparable_languages_public(
    enc_a: &str,
    enc_b: &str,
    languages: &HashSet<String>,
) -> Option<HashSet<String>> {
    comparable_languages(enc_a, enc_b, languages)
}

fn best_variant_score(
    profile: &BigramProfile,
    enc: &str,
    languages: Option<&HashSet<String>>,
) -> f64 {
    let md = model_data();
    let variants = match md.variants(enc) {
        Some(v) if !v.is_empty() => v,
        _ => return 0.0,
    };
    let mut best = 0.0f64;
    for &i in variants {
        let v = md.variant(i);
        if let Some(langs) = languages {
            match &v.lang {
                Some(l) if langs.contains(l) => {}
                _ => continue,
            }
        }
        let s = score_with_profile(profile, &v.table, &v.key);
        if s > best {
            best = s;
        }
    }
    best
}

/// Bigram profile of `data` restricted to bigrams touching `diff`.
pub fn build_focused_profile(data: &[u8], diff: &HashSet<u8>) -> Option<BigramProfile> {
    if data.len() < 2 {
        return None;
    }
    let (_non_diff, is_diff) = pair_byte_tables(diff);
    let hits = data.iter().filter(|b| diff.contains(b)).count();
    if hits == 0 {
        return None;
    }
    let idf = model_data().idf();
    let mut freq: HashMap<u32, u64> = HashMap::new();
    let limit = data.len() - 1;
    if hits * DENSE_HIT_DIVISOR < data.len() {
        let mut starts: HashSet<usize> = HashSet::new();
        let mut distinct = [false; 256];
        for &b in data {
            if diff.contains(&b) {
                distinct[b as usize] = true;
            }
        }
        for bv in 0..=255u8 {
            if !distinct[bv as usize] {
                continue;
            }
            let mut pos = 0usize;
            while pos < data.len() {
                match data[pos..].iter().position(|&x| x == bv) {
                    Some(p) => pos += p,
                    None => break,
                }
                if pos > 0 {
                    starts.insert(pos - 1);
                }
                if pos < limit {
                    starts.insert(pos);
                }
                pos += 1;
            }
        }
        for i in starts {
            let idx = ((data[i] as u32) << 8) | data[i + 1] as u32;
            *freq.entry(idx).or_insert(0) += idf[idx as usize] as u64;
        }
    } else {
        for i in 0..limit {
            let b1 = data[i];
            let b2 = data[i + 1];
            if !(is_diff[b1 as usize] | is_diff[b2 as usize]) {
                continue;
            }
            let idx = ((b1 as u32) << 8) | b2 as u32;
            *freq.entry(idx).or_insert(0) += idf[idx as usize] as u64;
        }
    }
    if freq.is_empty() {
        return None;
    }
    let items: Vec<(u32, u64)> = freq.into_iter().collect();
    Some(BigramProfile::from_weighted_freq(&items))
}

fn resolve_by_bigram_rescore(
    data: &[u8],
    enc_a: &str,
    enc_b: &str,
    diff: &HashSet<u8>,
    languages: &HashSet<String>,
) -> Option<String> {
    let profile = build_focused_profile(data, diff)?;
    let comparable = comparable_languages(enc_a, enc_b, languages);
    let best_a = best_variant_score(&profile, enc_a, comparable.as_ref());
    let best_b = best_variant_score(&profile, enc_b, comparable.as_ref());
    if best_a > best_b {
        Some(enc_a.to_string())
    } else if best_b > best_a {
        Some(enc_b.to_string())
    } else {
        None
    }
}

/// Byte values >= 0x80 that `enc_a` and `enc_b` decode to different text.
pub fn differing_high_bytes(enc_a: &str, enc_b: &str) -> HashSet<u8> {
    let mut out = HashSet::new();
    for b in 0x80..=0xFFu8 {
        let ta = decode_byte(enc_a, b);
        let tb = decode_byte(enc_b, b);
        if ta != tb {
            out.insert(b);
        }
    }
    out
}

fn pair_categories(enc_a: &str, enc_b: &str, diff: &HashSet<u8>) -> Categories {
    let mut table = Categories::new();
    for &b in diff {
        let mut cats = Vec::new();
        for enc in [enc_a, enc_b] {
            match decode_byte(enc, b) {
                Some(c) => cats.push(category_of_cp(c as u32).to_string()),
                None => cats.push("Cn".to_string()),
            }
        }
        table.insert(b, (cats[0].clone(), cats[1].clone()));
    }
    table
}

/// Decide a pair on its distinguishing bytes: model rescore, then context.
pub fn arbitrate_distinguishing_bytes(
    data: &[u8],
    enc_a: &str,
    enc_b: &str,
    diff: &HashSet<u8>,
    languages_a: Option<&HashSet<String>>,
    languages_b: Option<&HashSet<String>>,
) -> Option<String> {
    if let Some(profile) = build_focused_profile(data, diff) {
        let best_a = best_variant_score(&profile, enc_a, languages_a);
        let best_b = best_variant_score(&profile, enc_b, languages_b);
        if best_a > best_b {
            return Some(enc_a.to_string());
        }
        if best_b > best_a {
            return Some(enc_b.to_string());
        }
    }
    let cats = pair_categories(enc_a, enc_b, diff);
    vote_with_margin(data, enc_a, enc_b, diff, &cats).0
}

fn find_pair_key(maps: &DistMaps, a: &str, b: &str) -> Option<(String, String)> {
    if maps.contains_key(&(a.to_string(), b.to_string())) {
        return Some((a.to_string(), b.to_string()));
    }
    if maps.contains_key(&(b.to_string(), a.to_string())) {
        return Some((b.to_string(), a.to_string()));
    }
    None
}

pub const CROSS_FAMILY_MIN_DIFFS: usize = 52;
pub const CONFUSION_BAND: f64 = 0.005;
pub const CONFUSION_FLOOR_RATIO: f64 = 0.5;
pub const STRICT_TIER_MAX_CONF: f64 = 0.2;

/// Return the byte-evidence winner between two encodings, or `None`.
pub fn confusion_pair_winner(
    data: &[u8],
    enc_x: &str,
    enc_y: &str,
    languages: &HashSet<String>,
) -> Option<String> {
    let maps = load_confusion_data();
    let key = find_pair_key(maps, enc_x, enc_y)?;
    let (diff, categories) = &maps[&key];
    let (enc_a, enc_b) = key;
    let (cat_winner, _m, demotion_margin, demotion_events) =
        vote_with_margin(data, &enc_a, &enc_b, diff, categories);
    if cat_winner.is_some()
        && demotion_margin >= DECISIVE_VOTE_MARGIN
        && demotion_events >= DECISIVE_MIN_EVENTS
        && diff.len() < CROSS_FAMILY_MIN_DIFFS
    {
        return cat_winner;
    }
    let bigram_winner = resolve_by_bigram_rescore(data, &enc_a, &enc_b, diff, languages);
    if diff.len() >= CROSS_FAMILY_MIN_DIFFS {
        if bigram_winner.is_some() && bigram_winner == cat_winner {
            return bigram_winner;
        }
        return None;
    }
    bigram_winner.or(cat_winner)
}

/// Resolve confusion between similar encodings in the top results.
pub fn resolve_confusion_groups(
    data: &[u8],
    results: Vec<DetectionResult>,
) -> Vec<DetectionResult> {
    if results.len() < 2 {
        return results;
    }
    let top = results[0].clone();
    let top_enc = match &top.encoding {
        Some(e) => e.clone(),
        None => return results,
    };
    if top.language.as_deref() == Some(ART_LANGUAGE) {
        return results;
    }
    let maps = load_confusion_data();
    let top_conf = top.confidence;
    let floor = top_conf * CONFUSION_FLOOR_RATIO;

    let mut champion_idx = 0usize;
    let mut champion = top.clone();
    let mut champion_enc = top_enc.clone();

    let mut i = 1usize;
    while i < results.len() {
        let candidate = results[i].clone();
        let cand_enc = match &candidate.encoding {
            Some(e) => e.clone(),
            None => {
                i += 1;
                continue;
            }
        };
        let in_band = i == 1 || top_conf - candidate.confidence <= CONFUSION_BAND;
        if !in_band && (top_conf >= STRICT_TIER_MAX_CONF || candidate.confidence < floor) {
            break;
        }
        let (enc_a, enc_b) = match find_pair_key(maps, &champion_enc, &cand_enc) {
            Some(k) => k,
            None => {
                i += 1;
                continue;
            }
        };
        let (diff, categories) = &maps[&(enc_a.clone(), enc_b.clone())];
        let (cat_winner, _m, demotion_margin, demotion_events) =
            vote_with_margin(data, &enc_a, &enc_b, diff, categories);
        let winner: Option<String> = if cat_winner.is_some()
            && demotion_margin >= DECISIVE_VOTE_MARGIN
            && demotion_events >= DECISIVE_MIN_EVENTS
            && diff.len() < CROSS_FAMILY_MIN_DIFFS
        {
            cat_winner.clone()
        } else {
            let langs: HashSet<String> = [champion.language.clone(), candidate.language.clone()]
                .into_iter()
                .flatten()
                .collect();
            let bigram_winner =
                resolve_by_bigram_rescore(data, &enc_a, &enc_b, diff, &langs);
            if in_band && diff.len() < CROSS_FAMILY_MIN_DIFFS {
                bigram_winner.or_else(|| cat_winner.clone())
            } else if bigram_winner.is_some() && bigram_winner == cat_winner {
                bigram_winner
            } else {
                None
            }
        };

        if winner.is_none() || winner.as_deref() != Some(cand_enc.as_str()) {
            i += 1;
            continue;
        }
        if in_band {
            let promoted = DetectionResult::with_mime(
                Some(cand_enc.clone()),
                top_conf,
                candidate.language.clone(),
                candidate.mime_type.clone(),
            );
            let mut out = vec![promoted];
            for (j, r) in results.iter().enumerate() {
                if j != i {
                    out.push(r.clone());
                }
            }
            return out;
        }
        champion_idx = i;
        champion = candidate;
        champion_enc = cand_enc;
        i += 1;
    }

    if champion_idx == 0 {
        return results;
    }
    let promoted = DetectionResult::with_mime(
        champion.encoding.clone(),
        top_conf,
        champion.language.clone(),
        champion.mime_type.clone(),
    );
    let mut out = vec![promoted];
    for (j, r) in results.iter().enumerate() {
        if j != champion_idx {
            out.push(r.clone());
        }
    }
    out
}
