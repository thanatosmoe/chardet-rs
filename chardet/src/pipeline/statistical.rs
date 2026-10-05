//! Stage 3: Statistical bigram scoring (unpruned full ranking).

use crate::models::{score_best_language, BigramProfile};
use crate::registry::EncodingInfo;
use crate::result::DetectionResult;

/// Score all candidates and return results sorted by confidence descending.
pub fn score_candidates(
    data: &[u8],
    candidates: &[&EncodingInfo],
    _full_ranking: bool,
) -> Vec<DetectionResult> {
    if data.is_empty() || candidates.is_empty() {
        return Vec::new();
    }
    let profile = BigramProfile::new(data);
    if profile.input_norm == 0.0 {
        return Vec::new();
    }
    let mut scores: Vec<(String, f64, Option<String>)> = Vec::new();
    for enc in candidates {
        let (s, lang) = score_best_language(data, enc.name, Some(&profile), false);
        if s > 0.0 {
            scores.push((enc.name.to_string(), s, lang));
        }
    }
    // Stable sort by descending confidence (matches Python's stable sort).
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores
        .into_iter()
        .map(|(name, conf, lang)| DetectionResult::new(Some(name), conf, lang))
        .collect()
}
