use tree_sitter::QueryMatch;

use super::{ParsedMatch, definitions};

mod container;
mod enumeration;
mod field;
mod import;
mod orphan;
mod package;
mod rpc;

/// Routes a compiled Tree-sitter query match to its respective dedicated
/// extraction handler based on the matching root element flavor handle.
///
/// This dispatcher serves as the central hub of the extraction layer. If a
/// valid `kind_str` handle is provided, it matches it against known structural
/// definitions (e.g., packages, messages, fields, RPCs) and forwards the query
/// match payload to specialized modular extractors.
///
/// If the match lacks a root element handle, it falls back to parsing global
/// detached tokens (such as docstrings or isolated deprecation options).
///
/// # Arguments
///
/// * `kind_str` - An optional string identifier representing the matched
///   protobuf grammatical item wrapper.
/// * `query_match` - The raw capture match payload payload delivered from the
///   Tree-sitter query cursor runtime.
/// * `capture_names` - The chronological array of metadata capture handles
///   registered inside the compiled query instance.
/// * `source` - The raw UTF-8 byte array sequence containing the full original
///   file content on disk.
///
/// # Returns
///
/// Returns `Some(ParsedMatch)` enclosing the concrete extracted semantic graph
/// payload, or `None` if the match corresponds to an unknown definition type or
/// fails validation constraints.
#[inline]
pub fn dispatch_element(
    kind_str: Option<&str>,
    query_match: &QueryMatch,
    capture_names: &[&str],
    source: &[u8],
) -> Option<ParsedMatch> {
    let Some(kind_str) = kind_str else {
        return orphan::extract_orphan(query_match, capture_names, source);
    };

    match kind_str {
        definitions::PACKAGE => package::extract_package(query_match, capture_names, source),
        definitions::IMPORT => import::extract_import(query_match, capture_names, source),
        definitions::SERVICE => container::extract_service(query_match, capture_names, source),
        definitions::RPC => rpc::extract_rpc(query_match, capture_names, source),
        definitions::MESSAGE => container::extract_message(query_match, capture_names, source),
        definitions::FIELD => field::extract_field(query_match, capture_names, source),
        definitions::MAP_FIELD => field::extract_map_field(query_match, capture_names, source),
        definitions::ONEOF => container::extract_oneof(query_match, capture_names, source),
        definitions::ONEOF_FIELD => field::extract_oneof_field(query_match, capture_names, source),
        definitions::ENUM => enumeration::extract_enum(query_match, capture_names, source),
        definitions::ENUM_FIELD => {
            enumeration::extract_enum_field(query_match, capture_names, source)
        }

        _ => None,
    }
}
