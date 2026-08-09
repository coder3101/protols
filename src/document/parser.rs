use std::sync::Arc;

use async_lsp::lsp_types::{Range, Url};
use tree_sitter::{LanguageError, Parser, Query, Tree};

use crate::model::{ElementKind, ModelElement, SpatialEntry, build_meta_model};
pub struct ProtoParser {
    parser: tree_sitter::Parser,
}

#[derive(Clone)]
pub struct ProtoDocument {
    pub uri: Url,
    pub package: String,
    pub elements: Vec<ModelElement>,
    pub spatial_index: Vec<SpatialEntry>,
    pub tree: Arc<Tree>,
}

impl ProtoDocument {
    /// Attempts to parse a raw protobuf document and compile its optimized
    /// pure-memory metamodel and sorted spatial index.
    ///
    /// # Arguments
    ///
    /// * `uri` - The unique resource identifier ([`Url`]) representing the
    ///   document location.
    /// * `source` - The raw UTF-8 byte array sequence containing the complete
    ///   source file content.
    /// * `query` - The pre-compiled [`Query`] instance used for syntax
    ///   captures.
    /// * `ts_parser` - A mutable reference to the local instance of the native
    ///   Tree-sitter execution engine.
    ///
    /// # Returns
    ///
    /// Returns [`Some(ProtoDocument)`][ProtoDocument] housing the fully
    /// populated, search-ready graph cache layer, or [`None`] if the
    /// Tree-sitter runtime engine fails to initialize or parse the source.
    pub fn try_from_input(
        uri: Url,
        source: &[u8],
        query: &Query,
        ts_parser: &mut Parser,
    ) -> Option<Self> {
        let tree = ts_parser.parse(source, None)?;

        let (package, elements) = build_meta_model(tree.root_node(), source, query);

        let mut spatial_index = Vec::with_capacity(elements.len() * 2);

        for element in &elements {
            element.collect_spatial_entries(&mut spatial_index);
        }

        spatial_index.sort_by_key(|entry| entry.range.start);

        Some(Self {
            uri,
            package,
            elements,
            spatial_index,
            tree: Arc::new(tree),
        })
    }

    /// Returns the package namespace, defaulting to `"."` when undeclared.
    pub fn package_name(&self) -> &str {
        if self.package.is_empty() {
            "."
        } else {
            &self.package
        }
    }

    /// Returns the paths of all `import` statements declared in source.
    pub fn import_paths(&self) -> Vec<String> {
        self.elements
            .iter()
            .filter_map(|element| match &element.kind {
                ElementKind::Import { path } => Some(path.clone()),
                _ => None,
            })
            .collect()
    }

    /// Returns the ranges of import statements whose path is in `import`.
    pub fn import_path_ranges(&self, import: &[&str]) -> Vec<Range> {
        self.elements
            .iter()
            .filter_map(|element| match &element.kind {
                ElementKind::Import { path } if import.contains(&path.as_str()) => {
                    Some(element.meta.selection_range)
                }
                _ => None,
            })
            .collect()
    }
}

impl ProtoParser {
    pub fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();
        let trace_error = |e: &LanguageError| {
            tracing::error!(
                "Critical initialization failure: Failed to set Tree-sitter Protobuf language parser: {:?}",
                e
            );
        };

        parser
            .set_language(&tree_sitter_proto::LANGUAGE.into())
            .inspect_err(trace_error)
            .expect("Tree-sitter parser: setting language failed");

        Self { parser }
    }

    pub fn parse(
        &mut self,
        uri: Url,
        contents: impl AsRef<[u8]>,
        metamodel_query: &Query,
    ) -> Option<ProtoDocument> {
        ProtoDocument::try_from_input(uri, contents.as_ref(), metamodel_query, &mut self.parser)
    }
}
