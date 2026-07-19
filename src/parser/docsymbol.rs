//! Document Symbol hierarchy compilation layer for protobuf abstract syntax
//! trees.
//!
//! This module translates a flat, vector-backed metamodel registry into a fully
//! nested, tree-structured representation matching the LSP [`DocumentSymbol`]
//! specification.

use async_lsp::lsp_types::{DocumentSymbol, Range, SymbolKind, SymbolTag};

use crate::model::{ElementKind, ElementMeta, ModelElement};

use super::ParsedTree;

impl From<&ElementKind> for SymbolKind {
    /// Maps an internal [`ElementKind`] variant directly to its closest
    /// semantic LSP [`SymbolKind`].
    fn from(kind: &ElementKind) -> Self {
        match kind {
            ElementKind::Import { .. } => Self::MODULE,
            ElementKind::Message { .. } => Self::STRUCT,
            ElementKind::Oneof { .. } => Self::OBJECT,
            ElementKind::Field { .. }
            | ElementKind::MapField { .. }
            | ElementKind::OneofField { .. } => Self::FIELD,
            ElementKind::Enum { .. } => Self::ENUM,
            ElementKind::EnumValue { .. } => Self::ENUM_MEMBER,
            ElementKind::Service { .. } => Self::INTERFACE,
            ElementKind::Rpc { .. } => Self::METHOD,
        }
    }
}

impl ParsedTree {
    /// Compiles a fully resolved hierarchical tree of document symbols from the
    /// internal flat elements registry.
    ///
    /// # Returns
    ///
    /// A [`Vec<DocumentSymbol>`] sorted in the original top-down text order,
    /// ready for LSP serialization.
    pub fn document_symbols(&self) -> Vec<DocumentSymbol> {
        let mut symbols: Vec<Option<DocumentSymbol>> =
            self.elements.iter().map(create_document_symbol).collect();

        let mut root_symbols = Vec::new();

        for (index, element) in self.elements.iter().enumerate().rev() {
            let Some(mut symbol) = symbols.get_mut(index).and_then(Option::take) else {
                continue;
            };

            if let Some(children) = symbol.children.as_mut() {
                children.reverse();
            }

            let Some(parent_id) = element.parent_id else {
                root_symbols.push(symbol);
                continue;
            };

            let Some(parent_symbol) = symbols.get_mut(parent_id).and_then(Option::as_mut) else {
                root_symbols.push(symbol);
                continue;
            };

            if let Some(children) = &mut parent_symbol.children {
                children.push(symbol);
            }
        }

        root_symbols.into_iter().rev().collect()
    }
}

/// Factory function to assemble an un-linked [`DocumentSymbol`] instance from a
/// [`ModelElement`].
///
/// This constructor formats the `detail` string field with type references,
/// unique tags, and streaming prefixes (`→`) to match the display conventions
/// of modern language servers.
///
/// # Folding Range Optimization
///
/// If an element is prefixed with adjacent leading docstrings, this method
/// expands the physical `range` upwards to encompass the start line of the
/// first comment block. This guarantees that editor code-folding boundaries
/// cleanly wrap the documentation together with the block body.
fn create_document_symbol(element: &ModelElement) -> Option<DocumentSymbol> {
    if matches!(element.kind, ElementKind::Import { .. }) {
        return None;
    }

    let detail = match &element.kind {
        ElementKind::Field {
            type_ref,
            cardinality,
            tag,
            ..
        } => {
            let prefix = cardinality
                .as_ref()
                .map(|c| format!("{} ", c.kind))
                .unwrap_or_default();

            Some(format!("{prefix}{} (tag: {tag})", type_ref.name))
        }
        ElementKind::OneofField { type_ref, tag, .. } => {
            Some(format!("{} (tag: {tag})", type_ref.name))
        }
        ElementKind::MapField {
            key_type_ref,
            value_type_ref,
            tag,
            ..
        } => Some(format!(
            "map<{}, {}> (tag: {tag})",
            key_type_ref.name, value_type_ref.name
        )),
        ElementKind::Rpc {
            request_type_ref,
            request_stream,
            response_type_ref,
            response_stream,
            ..
        } => {
            let request_prefix = request_stream
                .as_ref()
                .map(|_| "stream ")
                .unwrap_or_default();
            let response_prefix = response_stream
                .as_ref()
                .map(|_| "stream ")
                .unwrap_or_default();

            Some(format!(
                "{request_prefix}{} → {response_prefix}{}",
                request_type_ref.name, response_type_ref.name
            ))
        }
        ElementKind::EnumValue { number, .. } => Some(format!("value: {number}")),
        _ => None,
    };

    let kind = SymbolKind::from(&element.kind);

    let tags = element
        .kind
        .is_deprecated()
        .then(|| vec![SymbolTag::DEPRECATED]);

    let range = element
        .meta
        .documentation
        .first()
        .map_or(element.meta.range, |c| Range {
            start: c.range.start,
            end: element.meta.range.end,
        });
    let ElementMeta {
        name,
        selection_range,
        ..
    } = &element.meta;

    #[allow(deprecated)]
    Some(DocumentSymbol {
        name: name.clone(),
        detail,
        kind,
        tags,
        range,
        selection_range: *selection_range,
        children: Some(Vec::new()),
        deprecated: None,
    })
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{DocumentSymbol, Url};
    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;

    fn run_symbols_test(contents: &str, file_name: &str) -> Vec<DocumentSymbol> {
        let uri = Url::parse(&format!("file:///virtual/{file_name}")).unwrap();
        let ipath = vec![];

        let mut state = ProtoLanguageState::new();
        state.upsert_file(
            &uri,
            contents.to_string(),
            &ipath,
            3,
            &Config::default(),
            false,
        );

        state
            .get_tree(&uri)
            .map(|tree| tree.document_symbols())
            .unwrap_or_default()
    }

    #[test]
    fn test_proto2_document_symbols() {
        let contents = include_str!("input/syntax_variants/test_proto2.proto");
        let symbols = run_symbols_test(contents, "test_proto2.proto");
        assert_yaml_snapshot!("test_proto2_document_symbols", symbols);
    }

    #[test]
    fn test_proto3_document_symbols() {
        let contents = include_str!("input/syntax_variants/test_proto3.proto");
        let symbols = run_symbols_test(contents, "test_proto3.proto");
        assert_yaml_snapshot!("test_proto3_document_symbols", symbols);
    }

    #[test]
    fn test_editions_document_symbols() {
        let contents = include_str!("input/syntax_variants/test_editions.proto");
        let symbols = run_symbols_test(contents, "test_editions.proto");
        assert_yaml_snapshot!("test_editions_document_symbols", symbols);
    }

    #[test]
    fn test_package_duplicate_document_symbols() {
        let contents = include_str!("input/syntax_variants/test_package_duplicate.proto");
        let symbols = run_symbols_test(contents, "test_package_duplicate.proto");
        assert_yaml_snapshot!("test_package_duplicate_document_symbols", symbols);
    }

    #[test]
    fn test_document_symbols_on_empty_and_minimal_file_safety() {
        let uri = Url::parse("file:///virtual/empty_test.proto").unwrap();
        let ipath = vec![];

        let mut state = ProtoLanguageState::new();
        state.upsert_file(&uri, String::new(), &ipath, 3, &Config::default(), false);

        let symbols = state
            .get_tree(&uri)
            .map(|tree| tree.document_symbols())
            .unwrap_or_default();

        assert!(symbols.is_empty());

        let mut state_minimal = ProtoLanguageState::new();
        state_minimal.upsert_file(
            &uri,
            "syntax = \"proto3\";\npackage com.test;".to_string(),
            &ipath,
            3,
            &Config::default(),
            false,
        );

        let symbols_minimal = state_minimal
            .get_tree(&uri)
            .map(|tree| tree.document_symbols())
            .unwrap_or_default();

        assert!(symbols_minimal.is_empty());
    }
}
