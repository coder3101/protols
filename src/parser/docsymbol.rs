use async_lsp::lsp_types::{DocumentSymbol, Range};
use tree_sitter::TreeCursor;

use crate::utils::ts_to_lsp_position;

use super::{ nodekind::NodeKind, ParsedTree};

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
        let should_pop = self.stack.last().map_or(false, |(n, _)| *n == node);
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
    use async_lsp::lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

    use crate::parser::ProtoParser;

    #[test]
    #[allow(deprecated)]
    fn test_document_symbols() {
        let contents = r#"syntax = "proto3";

package com.symbols;

// outer 1 comment
message Outer1 {
    message Inner1 {
        string name = 1;
    };

    Inner1 i = 1;
}

message Outer2 {
    message Inner2 {
        string name = 1;
    };
    // Inner 3 comment here
    message Inner3 {
        string name = 1;

        enum X {
            a = 1;
            b = 2;
        }
    }
    Inner1 i = 1;
    Inner2 y = 2;
}

"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let res = tree.find_document_locations(contents);

        assert_eq!(res.len(), 2);
        assert_eq!(
            res,
            vec!(
                DocumentSymbol {
                    name: "Outer1".to_string(),
                    detail: Some("outer 1 comment".to_string()),
                    kind: SymbolKind::STRUCT,
                    tags: None,
                    range: Range {
                        start: Position::new(5, 0),
                        end: Position::new(11, 1),
                    },
                    selection_range: Range {
                        start: Position::new(5, 8),
                        end: Position::new(5, 14),
                    },
                    children: Some(vec!(DocumentSymbol {
                        name: "Inner1".to_string(),
                        detail: None,
                        kind: SymbolKind::STRUCT,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position::new(6, 4),
                            end: Position::new(8, 5),
                        },
                        selection_range: Range {
                            start: Position::new(6, 12),
                            end: Position::new(6, 18),
                        },
                        children: Some(vec!()),
                    },)),
                    deprecated: None,
                },
                DocumentSymbol {
                    name: "Outer2".to_string(),
                    detail: None,
                    kind: SymbolKind::STRUCT,
                    tags: None,
                    range: Range {
                        start: Position::new(13, 0),
                        end: Position::new(28, 1),
                    },
                    selection_range: Range {
                        start: Position::new(13, 8),
                        end: Position::new(13, 14),
                    },
                    children: Some(vec!(
                        DocumentSymbol {
                            name: "Inner2".to_string(),
                            detail: None,
                            kind: SymbolKind::STRUCT,
                            tags: None,
                            deprecated: None,
                            range: Range {
                                start: Position::new(14, 4),
                                end: Position::new(16, 5),
                            },
                            selection_range: Range {
                                start: Position::new(14, 12),
                                end: Position::new(14, 18),
                            },
                            children: Some(vec!()),
                        },
                        DocumentSymbol {
                            name: "Inner3".to_string(),
                            detail: Some("Inner 3 comment here".to_string()),
                            kind: SymbolKind::STRUCT,
                            tags: None,
                            deprecated: None,
                            range: Range {
                                start: Position::new(18, 4),
                                end: Position::new(25, 5),
                            },
                            selection_range: Range {
                                start: Position::new(18, 12),
                                end: Position::new(18, 18),
                            },
                            children: Some(vec!(DocumentSymbol {
                                name: "X".to_string(),
                                detail: None,
                                kind: SymbolKind::ENUM,
                                tags: None,
                                deprecated: None,
                                range: Range {
                                    start: Position::new(21, 8),
                                    end: Position::new(24, 9),
                                },
                                selection_range: Range {
                                    start: Position::new(21, 13),
                                    end: Position::new(21, 14),
                                },
                                children: Some(vec!()),
                            })),
                        }
                    )),
                    deprecated: None,
                },
            )
        );
    }
}
