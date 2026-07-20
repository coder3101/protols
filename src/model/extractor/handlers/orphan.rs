use std::str::Utf8Error;

use tree_sitter::{QueryCapture, QueryMatch};

use crate::model::CommentBlock;
use crate::utils::{clean_proto_comment, to_lsp_range};

use super::super::captures::{definitions, properties};
use super::ParsedMatch;

#[inline]
pub(super) fn extract_orphan(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::DOC_COMMENT => {
                let trace_utf8_error = |e: &Utf8Error| {
                    tracing::warn!(
                        "Orphan comment block skipped: failed to extract valid UTF-8 text at {:?}. Error: {:?}",
                        to_lsp_range(node),
                        e
                    );
                };

                if let Ok(raw_text) = node.utf8_text(source).inspect_err(trace_utf8_error) {
                    return Some(ParsedMatch::Comment(CommentBlock {
                        text: clean_proto_comment(raw_text),
                        range: to_lsp_range(node),
                    }));
                }
            }
            properties::OPTION_NAME | properties::OPTION_VALUE => {}
            properties::DEPRECATION_MARKER => {
                return Some(ParsedMatch::DeprecationMarker {
                    range: to_lsp_range(node),
                });
            }
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "handle_orphan_match: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside handle_orphan_match at {:?}",
                    unknown,
                    to_lsp_range(node)
                );
            }
        }
    }

    None
}
