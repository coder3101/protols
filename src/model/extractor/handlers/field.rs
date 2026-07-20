use std::num::ParseIntError;
use std::str::Utf8Error;

use tree_sitter::{QueryCapture, QueryMatch};

use crate::model::{ElementKind, FieldCardinality, TypeReference};
use crate::utils::to_lsp_range;

use super::super::captures::{definitions, properties, references};
use super::ParsedMatch;

#[inline]
pub(super) fn extract_field(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut type_ref = TypeReference::with_capacity(32);
    let mut name_ref = TypeReference::with_capacity(32);
    let mut tag = 0u32;
    let mut range = None;
    let mut block_text = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            references::FIELD_TYPE => type_ref.fill(node, source),
            properties::NAME => name_ref.fill(node, source),
            properties::TAG => {
                let trace_utf8_error = |e: &Utf8Error| {
                    tracing::warn!(
                        "Field tag value skipped: failed to extract valid UTF-8 text at {:?}. Error: {:?}",
                        to_lsp_range(node),
                        e
                    );
                };
                if let Ok(raw) = node.utf8_text(source).inspect_err(trace_utf8_error) {
                    let trace_number_error = |e: &ParseIntError| {
                        tracing::warn!(
                            "Soft validation: failed to parse field tag number '{}' at {:?}. Details: {:?}",
                            raw,
                            to_lsp_range(node),
                            e
                        );
                    };
                    tag = raw
                        .parse::<u32>()
                        .inspect_err(trace_number_error)
                        .unwrap_or(0);
                }
            }
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            definitions::FIELD => {
                range = Some(to_lsp_range(node));
                let trace_utf8_error = |e: &Utf8Error| {
                    tracing::error!(
                        "Field text value skipped: failed to extract valid UTF-8 text at {:?}. Error: {:?}",
                        to_lsp_range(node),
                        e
                    );
                };
                block_text = node.utf8_text(source).inspect_err(trace_utf8_error).ok();
            }
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_field: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_field at {:?}",
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
        tracing::error!("extract_field: failed to extract field name");
        return None;
    }

    if block_text.is_none() {
        tracing::error!("extract_field: failed to extract valid block text");
    }

    if range.is_none() {
        tracing::error!("extract_field: failed to extract valid block range");
    }

    let block_text = block_text?;
    let range = range?;

    let cardinality = FieldCardinality::from_block_text(block_text, range);

    Some(ParsedMatch::Entity {
        kind: ElementKind::Field {
            fqn,
            type_ref,
            cardinality,
            tag,
            is_deprecated: false,
        },
        range,
        selection_range,
    })
}

#[inline]
pub(super) fn extract_map_field(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut key_type_ref = TypeReference::with_capacity(32);
    let mut value_type_ref = TypeReference::with_capacity(32);
    let mut name_ref = TypeReference::with_capacity(32);
    let mut tag = 0u32;
    let mut range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            references::MAP_KEY => key_type_ref.fill(node, source),
            references::MAP_VALUE => value_type_ref.fill(node, source),
            properties::NAME => name_ref.fill(node, source),
            properties::TAG => {
                let trace_utf8_error = |e: &Utf8Error| {
                    tracing::warn!(
                        "Map field tag value skipped: failed to extract valid UTF-8 text at {:?}. Error: {:?}",
                        to_lsp_range(node),
                        e
                    );
                };
                if let Ok(raw) = node.utf8_text(source).inspect_err(trace_utf8_error) {
                    let trace_number_error = |e: &ParseIntError| {
                        tracing::warn!(
                            "Soft validation: failed to parse map field tag number '{}' at {:?}. Details: {:?}",
                            raw,
                            to_lsp_range(node),
                            e
                        );
                    };
                    tag = raw
                        .parse::<u32>()
                        .inspect_err(trace_number_error)
                        .unwrap_or(0);
                }
            }
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            definitions::MAP_FIELD => range = Some(to_lsp_range(node)),
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_map_field: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_map_field at {:?}",
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
        tracing::error!("extract_map_field: failed to extract map field name");
        return None;
    }

    if key_type_ref.name.is_empty() {
        tracing::error!("extract_map_field: failed to extract map key name");
        return None;
    }

    if value_type_ref.name.is_empty() {
        tracing::error!("extract_map_field: failed to extract map value name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_map_field: failed to extract valid block range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::MapField {
            fqn,
            key_type_ref,
            value_type_ref,
            tag,
            is_deprecated: false,
        },
        range,
        selection_range,
    })
}

#[inline]
pub(super) fn extract_oneof_field(
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let mut type_ref = TypeReference::with_capacity(32);
    let mut name_ref = TypeReference::with_capacity(32);
    let mut tag = 0u32;
    let mut range = None;

    for QueryCapture { node, index } in query_match.captures.iter().copied() {
        let capture_name = capture_names[index as usize];

        match capture_name {
            references::FIELD_TYPE => type_ref.fill(node, source),
            properties::NAME => name_ref.fill(node, source),
            properties::TAG => {
                let trace_utf8_error = |e: &Utf8Error| {
                    tracing::warn!(
                        "Oneof field tag value skipped: failed to extract valid UTF-8 text at {:?}. Error: {:?}",
                        to_lsp_range(node),
                        e
                    );
                };
                if let Ok(raw) = node.utf8_text(source).inspect_err(trace_utf8_error) {
                    let trace_number_error = |e: &ParseIntError| {
                        tracing::warn!(
                            "Soft validation: failed to parse oneof field tag number '{}' at {:?}. Details: {:?}",
                            raw,
                            to_lsp_range(node),
                            e
                        );
                    };
                    tag = raw
                        .parse::<u32>()
                        .inspect_err(trace_number_error)
                        .unwrap_or(0);
                }
            }
            properties::OPTION_NAME | properties::OPTION_VALUE | properties::DOC_COMMENT => {}
            definitions::ONEOF_FIELD => range = Some(to_lsp_range(node)),
            invalid_kind if definitions::is_match(invalid_kind) => {
                tracing::error!(
                    "extract_oneof_field: received an incompatible element capture '{}' at range {:?}",
                    invalid_kind,
                    to_lsp_range(node)
                );
            }
            unknown => {
                tracing::debug!(
                    "Unused auxiliary capture '{}' ignored inside extract_oneof_field at {:?}",
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
        tracing::error!("extract_oneof_field: failed to extract oneof field name");
        return None;
    }

    if range.is_none() {
        tracing::error!("extract_oneof_field: failed to extract valid block range");
    }

    let range = range?;

    Some(ParsedMatch::Entity {
        kind: ElementKind::OneofField {
            fqn,
            is_deprecated: false,
            type_ref,
            tag,
        },
        range,
        selection_range,
    })
}
