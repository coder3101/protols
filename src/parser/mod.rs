use std::sync::Arc;

use async_lsp::lsp_types::Url;
use tree_sitter::Tree;

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
    tree: Arc<Tree>,
}

impl ProtoParser {
    pub fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();
        if let Err(e) = parser.set_language(&protols_tree_sitter_proto::language()) {
            panic!("failed to set ts language parser {:?}", e);
        }
        Self { parser }
    }

    pub fn parse(&mut self, uri: Url, contents: impl AsRef<[u8]>) -> Option<ParsedTree> {
        self.parser.parse(contents, None).map(|t| ParsedTree {
            tree: Arc::new(t),
            uri,
        })
    }
}
