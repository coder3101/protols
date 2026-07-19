use tree_sitter::{QueryCapture, QueryMatch};

use crate::model::TypeReference;
use crate::utils::to_lsp_range;

use super::super::captures::{definitions, properties};
use super::ParsedMatch;

#[inline]
pub(super) fn extract_package(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut name_ref = TypeReference::with_capacity(32);

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::NAME => name_ref.fill(node, source),
            definitions::PACKAGE | properties::DOC_COMMENT => {}
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_package: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_package at {:?}",
                    unknown,
                    to_lsp_range(node)
                );
            }
        }
    }

    let TypeReference { name, .. } = name_ref;

    if name.is_empty() {
        tracing::error!("extract_package: failed to extract package name");
        return None;
    }

    Some(ParsedMatch::Package { name })
}
