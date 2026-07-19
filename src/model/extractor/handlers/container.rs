use tree_sitter::{QueryCapture, QueryMatch};

use crate::model::{ElementKind, TypeReference};
use crate::utils::to_lsp_range;

use super::super::captures::{definitions, properties};
use super::ParsedMatch;

#[inline]
pub(super) fn extract_service(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut name_ref = TypeReference::with_capacity(32);
    let mut range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::NAME => name_ref.fill(node, source),
            definitions::SERVICE => range = Some(to_lsp_range(node)),
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_service: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_service at {:?}",
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
        tracing::error!("extract_service: failed to extract service name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_service: failed to extract valid block range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::Service {
            fqn,
            is_deprecated: false,
        },
        range,
        selection_range,
    })
}

#[inline]
pub(super) fn extract_message(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut name_ref = TypeReference::with_capacity(32);
    let mut range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::NAME => name_ref.fill(node, source),
            definitions::MESSAGE => range = Some(to_lsp_range(node)),
            properties::OPTION_NAME
            | properties::OPTION_VALUE
            | properties::DOC_COMMENT
            | properties::DEPRECATION_MARKER => {}
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_message: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_message at {:?}",
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
        tracing::error!("extract_message: failed to extract message name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_message: failed to extract valid block range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::Message {
            fqn,
            is_deprecated: false,
        },
        range,
        selection_range,
    })
}

#[inline]
pub(super) fn extract_oneof(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut name_ref = TypeReference::with_capacity(32);
    let mut range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::NAME => name_ref.fill(node, source),
            definitions::ONEOF => range = Some(to_lsp_range(node)),
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_oneof: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_oneof at {:?}",
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
        tracing::error!("extract_oneof: failed to extract oneof name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_oneof: failed to extract valid block range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::Oneof { fqn },
        range,
        selection_range,
    })
}
