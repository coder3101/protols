use async_lsp::lsp_types::{DocumentSymbol, Range};
use tree_sitter::TreeCursor;

use crate::{nodekind::NodeKind, utils::ts_to_lsp_position};

use super::ParsedTree;

#[derive(Default)]
pub(super) struct DocumentSymbolTreeBuilder {
    // The stack are things we're still in the process of building/parsing.
    stack: Vec<(usize, DocumentSymbol)>,
    // The found are things we've finished processing/parsing, at the top level of the stack.
    found: Vec<DocumentSymbol>,
}

impl DocumentSymbolTreeBuilder {
    pub(super) fn push(&mut self, node: usize, symbol: DocumentSymbol) {
        self.stack.push((node, symbol));
    }

    pub(super) fn maybe_pop(&mut self, node: usize) {
        let should_pop = self.stack.last().is_some_and(|(n, _)| *n == node);
        if should_pop {
            let (_, explored) = self.stack.pop().unwrap();
            if let Some((_, parent)) = self.stack.last_mut() {
                parent.children.as_mut().unwrap().push(explored);
            } else {
                self.found.push(explored);
            }
        }
    }

    pub(super) fn build(self) -> Vec<DocumentSymbol> {
        self.found
    }
}

impl ParsedTree {
    pub fn find_document_locations(&self, content: impl AsRef<[u8]>) -> Vec<DocumentSymbol> {
        let mut builder = DocumentSymbolTreeBuilder::default();
        let content = content.as_ref();

        let mut cursor = self.tree.root_node().walk();
        self.find_document_locations_inner(&mut builder, &mut cursor, content);

        builder.build()
    }

    fn find_document_locations_inner(
        &self,
        builder: &mut DocumentSymbolTreeBuilder,
        cursor: &'_ mut TreeCursor,
        content: &[u8],
    ) {
        loop {
            let node = cursor.node();

            if NodeKind::is_userdefined(&node) {
                let name = node.utf8_text(content).unwrap();
                let kind = NodeKind::to_symbolkind(&node);
                let detail = self.find_preceding_comments(node.id(), content);
                let message = node.parent().unwrap();

                // https://github.com/rust-lang/rust/issues/102777
                #[allow(deprecated)]
                let new_symbol = DocumentSymbol {
                    name: name.to_string(),
                    detail,
                    kind,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: ts_to_lsp_position(&message.start_position()),
                        end: ts_to_lsp_position(&message.end_position()),
                    },
                    selection_range: Range {
                        start: ts_to_lsp_position(&node.start_position()),
                        end: ts_to_lsp_position(&node.end_position()),
                    },
                    children: Some(vec![]),
                };

                builder.push(message.id(), new_symbol);
            }

            if cursor.goto_first_child() {
                self.find_document_locations_inner(builder, cursor, content);
                builder.maybe_pop(node.id());
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use insta::assert_yaml_snapshot;

    use crate::parser::ProtoParser;

    #[test]
    #[allow(deprecated)]
    fn test_document_symbols() {
        let uri: Url = "file://foo/bar/pro.proto".parse().unwrap();
        let contents = include_str!("input/test_document_symbols.proto");

        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());

        let tree = parsed.unwrap();
        assert_yaml_snapshot!(tree.find_document_locations(contents));
    }
}
