//! Three-tier language detection, mirroring `chardet.pipeline.language`.

use crate::codecs::decode_to_utf8_lossy;
use crate::models::{
    model_data, score_best_language, BigramProfile, ART_LANGUAGE, RARE_LANGUAGES,
    THIN_RARE_MAX_BYTES,
};
use crate::result::DetectionResult;

const LANG_SCORE_MAX_BYTES: usize = 2048;

fn has_model_variants(encoding: &str) -> bool {
    model_data().has_variants(encoding)
}

/// Fill missing `language` fields via the three-tier algorithm.
pub fn fill_languages(data: &[u8], results: Vec<DetectionResult>) -> Vec<DetectionResult> {
    let data = &data[..data.len().min(LANG_SCORE_MAX_BYTES)];
    let thin = !data.is_empty() && data.len() < THIN_RARE_MAX_BYTES;
    let mut filled = Vec::with_capacity(results.len());
    let mut profile: Option<BigramProfile> = None;
    let mut utf8_profile: Option<BigramProfile> = None;
    let mut utf8_src: Option<Vec<u8>> = None;

    for result in results {
        let recheck = thin
            && result
                .language
                .as_deref()
                .map(|l| RARE_LANGUAGES.contains(&l))
                .unwrap_or(false)
            && result.encoding.is_some();
        if result.encoding.is_none() || (result.language.is_some() && !recheck) {
            filled.push(result);
            continue;
        }
        let encoding = result.encoding.clone().unwrap();
        let mut lang: Option<String> = if recheck {
            None
        } else {
            model_data()
                .infer_language(&encoding)
                .map(|s| s.to_string())
        };
        if lang.is_none() && !data.is_empty() && has_model_variants(&encoding) {
            if profile.is_none() {
                profile = Some(BigramProfile::new(data));
            }
            let (_, l) = score_best_language(data, &encoding, profile.as_ref(), thin);
            lang = l;
        }
        let escalate = thin
            && lang
                .as_deref()
                .map(|l| RARE_LANGUAGES.contains(&l))
                .unwrap_or(false);
        if (lang.is_none() || escalate) && !data.is_empty() && has_model_variants("utf-8") {
            if let Some(utf8_data) = decode_to_utf8_lossy(data, &encoding) {
                if utf8_src.as_deref() != Some(utf8_data.as_slice()) {
                    utf8_profile = Some(BigramProfile::new(&utf8_data));
                    utf8_src = Some(utf8_data.clone());
                }
                let (_, utf8_lang) =
                    score_best_language(&utf8_data, "utf-8", utf8_profile.as_ref(), thin);
                let utf8_ok = utf8_lang
                    .as_deref()
                    .map(|l| !RARE_LANGUAGES.contains(&l))
                    .unwrap_or(false);
                if lang.is_none() || utf8_ok {
                    lang = utf8_lang;
                }
            }
        }
        let rare_lang = lang
            .as_deref()
            .map(|l| RARE_LANGUAGES.contains(&l))
            .unwrap_or(false);
        if lang.is_none() || (recheck && rare_lang) {
            filled.push(result);
        } else {
            filled.push(DetectionResult::with_mime(
                Some(encoding),
                result.confidence,
                lang,
                result.mime_type.clone(),
            ));
        }
    }
    filled
}

/// Expose the art pseudo-language for callers.
pub const ART_LANG: &str = ART_LANGUAGE;
