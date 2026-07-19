use std::num::ParseIntError;
use std::str::Utf8Error;

use tree_sitter::{QueryCapture, QueryMatch};

use crate::model::{ElementKind, TypeReference};
use crate::utils::to_lsp_range;

use super::super::captures::{definitions, properties};
use super::ParsedMatch;

#[inline]
pub(super) fn extract_enum(
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
            definitions::ENUM => range = Some(to_lsp_range(node)),
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_enum: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_enum at {:?}",
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
        tracing::error!("extract_enum: failed to extract enum name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_enum: failed to extract valid block range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::Enum {
            fqn,
            is_deprecated: false,
        },
        range,
        selection_range,
    })
}

#[inline]
pub(super) fn extract_enum_field(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut name_ref = TypeReference::with_capacity(32);
    let mut number = 0i32;
    let mut range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            properties::NAME => name_ref.fill(node, source),
            properties::ENUM_VALUE => {
                let trace_utf8_error = |e: &Utf8Error| {
                    tracing::warn!(
                        "Enum field value skipped: failed to extract valid UTF-8 text at {:?}. Error: {:?}",
                        to_lsp_range(node),
                        e
                    );
                };

                if let Ok(raw) = node.utf8_text(source).inspect_err(trace_utf8_error) {
                    let trace_number_error = |e: &ParseIntError| {
                        tracing::warn!(
                            "Soft validation: failed to parse enum constant number '{}' at {:?}. Details: {:?}",
                            raw,
                            to_lsp_range(node),
                            e
                        );
                    };
                    number = raw
                        .parse::<i32>()
                        .inspect_err(trace_number_error)
                        .unwrap_or(0);
                }
            }
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            definitions::ENUM_FIELD => range = Some(to_lsp_range(node)),
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_enum_field: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_enum_field at {:?}",
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
        tracing::error!("extract_enum_field: failed to extract enum field name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_enum_field: failed to extract valid block range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::EnumValue {
            fqn,
            number,
            is_deprecated: false,
        },
        range,
        selection_range,
    })
}
