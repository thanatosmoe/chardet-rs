//! Stage 2a: Byte sequence validity filtering.

use crate::codecs::decodes_without_error;
use crate::registry::EncodingInfo;

/// Filter candidates to only those where `data` decodes without errors.
pub fn filter_by_validity<'a>(
    data: &[u8],
    candidates: &[&'a EncodingInfo],
) -> Vec<&'a EncodingInfo> {
    if data.is_empty() {
        return candidates.to_vec();
    }
    candidates
        .iter()
        .filter(|enc| decodes_without_error(data, enc.name))
        .copied()
        .collect()
}
