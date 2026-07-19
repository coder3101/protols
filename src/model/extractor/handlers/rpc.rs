use tree_sitter::{QueryCapture, QueryMatch};

use crate::model::{ElementKind, StreamModifier, TypeReference};
use crate::utils::to_lsp_range;

use super::super::captures::{definitions, properties, references};
use super::ParsedMatch;

#[inline]
pub(super) fn extract_rpc(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut name_ref = TypeReference::with_capacity(32);
    let mut request_stream = None;
    let mut request_type_ref = TypeReference::with_capacity(32);
    let mut response_stream = None;
    let mut response_type_ref = TypeReference::with_capacity(32);
    let mut range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::NAME => name_ref.fill(node, source),
            properties::RPC_REQUEST_STREAM => {
                request_stream = Some(StreamModifier {
                    range: to_lsp_range(node),
                });
            }
            references::RPC_REQUEST => request_type_ref.fill(node, source),
            properties::RPC_RESPONSE_STREAM => {
                response_stream = Some(StreamModifier {
                    range: to_lsp_range(node),
                });
            }
            references::RPC_RESPONSE => response_type_ref.fill(node, source),
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            definitions::RPC => range = Some(to_lsp_range(node)),
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_rpc: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_rpc at {:?}",
                    unknown,
                    to_lsp_range(node)
                );
            }
        }
    }

    let TypeReference {
        name: fqn,
        range: selection_range,
    } = name_ref;

    if fqn.is_empty() {
        tracing::error!("extract_rpc failed to extract RPC name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_rpc failed to extract RPC range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::Rpc {
            fqn,
            request_type_ref,
            request_stream,
            response_type_ref,
            response_stream,
            is_deprecated: false,
        },
        range,
        selection_range,
    })
}
