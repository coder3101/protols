use std::str::Utf8Error;

use tree_sitter::{QueryCapture, QueryMatch};

use crate::model::ElementKind;
use crate::utils::to_lsp_range;

use super::super::captures::{definitions, properties};
use super::ParsedMatch;

#[inline]
pub(super) fn extract_import(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut path = None;
    let mut range = None;
    let mut path_range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::IMPORT_PATH => {
                // The string node includes quotes; the clickable range should not.
                let mut inner = to_lsp_range(node);
                inner.start.character = inner.start.character.saturating_add(1);
                inner.end.character = inner.end.character.saturating_sub(1);
                path_range = Some(inner);
                let trace_utf8_error = |e: &Utf8Error| {
                    tracing::error!(
                        "extract_import: failed to extract valid UTF-8 text as import path at {:?}. Error: {:?}",
                        to_lsp_range(node),
                        e
                    );
                };
                path = node
                    .utf8_text(source)
                    .map(|raw| raw.trim_matches(['"', '\'']).to_string())
                    .inspect_err(trace_utf8_error)
                    .ok();
            }
            definitions::IMPORT => range = Some(to_lsp_range(node)),
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_import received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_import at {:?}",
                    unknown,
                    to_lsp_range(node)
                );
            }
        }
    }

    if path.is_none() {
        tracing::error!("extract_import: failed to extract valid path");
    }

    if range.is_none() {
        tracing::error!("extract_import: failed to extract valid block range");
    }

    let path = path?;
    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::Import { path },
        range,
        selection_range: path_range?,
    })
}
