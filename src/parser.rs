use std::sync::Arc;

use async_lsp::lsp_types::Url;
use tree_sitter::{Parser, Query, Tree};

use crate::model::{ModelElement, SpatialEntry, build_meta_model};

mod definition;
mod diagnostics;
mod docsymbol;
mod hover;
mod rename;
mod tree;

pub struct ProtoParser {
    parser: tree_sitter::Parser,
}

#[derive(Clone)]
pub struct ParsedTree {
    pub uri: Url,
    pub package: String,
    pub elements: Vec<ModelElement>,
    pub spatial_index: Vec<SpatialEntry>,
    tree: Arc<Tree>,
}

impl ParsedTree {
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
    /// Returns `Some(ParsedTree)` housing the fully populated, search-ready
    /// graph cache layer, or `None` if the Tree-sitter runtime engine fails to
    /// initialize or parse the source.
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
}

impl ProtoParser {
    pub fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();

        if let Err(e) = parser.set_language(&tree_sitter_proto::LANGUAGE.into()) {
            tracing::error!(
                "Critical initialization failure: Failed to set Tree-sitter Protobuf language parser: {:?}",
                e
            );

            std::thread::sleep(std::time::Duration::from_millis(50));
            std::process::exit(1);
        }

        Self { parser }
    }

    pub fn parse(
        &mut self,
        uri: Url,
        contents: impl AsRef<[u8]>,
        metamodel_query: &Query,
    ) -> Option<ParsedTree> {
        ParsedTree::try_from_input(uri, contents.as_ref(), metamodel_query, &mut self.parser)
    }
}
