use std::{collections::HashMap, unreachable};

use async_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, DocumentSymbol, Location, MarkedString, Position,
    PublishDiagnosticsParams, Range, SymbolKind, TextEdit, Url, WorkspaceEdit,
};
use tracing::info;
use tree_sitter::{Node, Tree, TreeCursor};

use crate::{
    utils::{lsp_to_ts_point, ts_to_lsp_position},
    wellknown,
};

pub struct ProtoParser {
    parser: tree_sitter::Parser,
}

pub struct ParsedTree {
    tree: Tree,
}

// Adding any new kind to USER_DEFINED_KINDS must be accompanied with a change in DocumentSymbol
// handler match statement.
const USER_DEFINED_KINDS: &[&str] = &["message_name", "enum_name"];
const ACTIONABLE_KINDS: &[&str] = &[
    "message_name",
    "enum_name",
    "message_or_enum_type",
    "rpc_name",
    "service_name",
];

#[derive(Default)]
struct DocumentSymbolTreeBuilder {
    // The stack are things we're still in the process of building/parsing.
    stack: Vec<(usize, DocumentSymbol)>,
    // The found are things we've finished processing/parsing, at the top level of the stack.
    found: Vec<DocumentSymbol>,
}

impl DocumentSymbolTreeBuilder {
    fn push(&mut self, node: usize, symbol: DocumentSymbol) {
        self.stack.push((node, symbol));
    }

    fn maybe_pop(&mut self, node: usize) {
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

    fn build(self) -> Vec<DocumentSymbol> {
        self.found
    }
}

impl ProtoParser {
    pub fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();
        if let Err(e) = parser.set_language(&protols_tree_sitter_proto::language()) {
            panic!("failed to set ts language parser {:?}", e);
        }
        Self { parser }
    }

    pub fn parse(&mut self, contents: impl AsRef<[u8]>) -> Option<ParsedTree> {
        self.parser
            .parse(contents, None)
            .map(|t| ParsedTree { tree: t })
    }
}

impl ParsedTree {
    fn walk_and_collect_kinds<'a>(cursor: &mut TreeCursor<'a>, kinds: &[&str]) -> Vec<Node<'a>> {
        let mut v = vec![];

        loop {
            let node = cursor.node();

            if kinds.contains(&node.kind()) {
                v.push(node)
            }

            if cursor.goto_first_child() {
                v.extend(Self::walk_and_collect_kinds(cursor, kinds));
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        v
    }

    fn advance_cursor_to(cursor: &mut TreeCursor<'_>, nid: usize) -> bool {
        loop {
            let node = cursor.node();
            if node.id() == nid {
                return true;
            }
            if cursor.goto_first_child() {
                if Self::advance_cursor_to(cursor, nid) {
                    return true;
                }
                cursor.goto_parent();
            }
            if !cursor.goto_next_sibling() {
                return false;
            }
        }
    }

    fn find_preceding_comments(&self, nid: usize, content: impl AsRef<[u8]>) -> Option<String> {
        let root = self.tree.root_node();
        let mut cursor = root.walk();

        Self::advance_cursor_to(&mut cursor, nid);
        if !cursor.goto_parent() {
            return None;
        }

        if !cursor.goto_previous_sibling() {
            return None;
        }

        let mut comments = vec![];
        while cursor.node().kind() == "comment" {
            let node = cursor.node();
            let text = node
                .utf8_text(content.as_ref())
                .expect("utf-8 parser error")
                .trim()
                .trim_start_matches("//")
                .trim();

            comments.push(text);

            if !cursor.goto_previous_sibling() {
                break;
            }
        }
        if !comments.is_empty() {
            comments.reverse();
            Some(comments.join("\n"))
        } else {
            None
        }
    }
}

impl ParsedTree {
    pub fn get_node_text_at_position<'a>(
        &'a self,
        pos: &Position,
        content: &'a [u8],
    ) -> Option<&'a str> {
        self.get_node_at_position(pos)
            .map(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error"))
    }

    pub fn get_actionable_node_text_at_position<'a>(
        &'a self,
        pos: &Position,
        content: &'a [u8],
    ) -> Option<&'a str> {
        self.get_actionable_node_at_position(pos)
            .map(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error"))
    }

    pub fn get_actionable_node_at_position<'a>(&'a self, pos: &Position) -> Option<Node<'a>> {
        self.get_node_at_position(pos)
            .map(|n| {
                if ACTIONABLE_KINDS.contains(&n.kind()) {
                    n
                } else {
                    n.parent().unwrap()
                }
            })
            .filter(|n| ACTIONABLE_KINDS.contains(&n.kind()))
    }

    pub fn get_node_at_position<'a>(&'a self, pos: &Position) -> Option<Node<'a>> {
        let pos = lsp_to_ts_point(pos);
        self.tree.root_node().descendant_for_point_range(pos, pos)
    }

    pub fn find_childrens_by_kinds(&self, kinds: &[&str]) -> Vec<Node> {
        let mut cursor = self.tree.root_node().walk();
        Self::walk_and_collect_kinds(&mut cursor, kinds)
    }

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

            if USER_DEFINED_KINDS.contains(&node.kind()) {
                let name = node.utf8_text(content).unwrap();
                let kind = match node.kind() {
                    "message_name" => SymbolKind::STRUCT,
                    "enum_name" => SymbolKind::ENUM,
                    _ => unreachable!("unsupported symbol kind"),
                };
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

    pub fn definition(
        &self,
        pos: &Position,
        uri: &Url,
        content: impl AsRef<[u8]>,
    ) -> Vec<Location> {
        let text = self.get_node_text_at_position(pos, content.as_ref());
        info!("Looking for definition of: {:?}", text);

        match text {
            Some(text) => self
                .find_childrens_by_kinds(USER_DEFINED_KINDS)
                .into_iter()
                .filter(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error") == text)
                .map(|n| Location {
                    uri: uri.clone(),
                    range: Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    },
                })
                .collect(),
            None => vec![],
        }
    }

    pub fn hover(&self, pos: &Position, content: impl AsRef<[u8]>) -> Vec<MarkedString> {
        let text = self.get_actionable_node_text_at_position(pos, content.as_ref());
        info!("Looking for hover response on: {:?}", text);

        match text {
            Some(text) => self
                .find_childrens_by_kinds(ACTIONABLE_KINDS)
                .into_iter()
                .filter(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error") == text)
                .filter_map(|n| {
                    self.find_preceding_comments(n.id(), content.as_ref())
                        .or_else(|| {
                            wellknown::hover_on(
                                n.utf8_text(content.as_ref()).expect("utf-8 parse error"),
                            )
                            .map(ToOwned::to_owned)
                        })
                })
                .map(MarkedString::String)
                .collect(),
            None => vec![],
        }
    }

    pub fn can_rename(&self, pos: &Position) -> Option<Range> {
        self.get_node_at_position(pos)
            .filter(|n| n.kind() == "identifier")
            .map(|n| n.parent().unwrap()) // Safety: Identifier must have a parent node
            .filter(|n| ACTIONABLE_KINDS.contains(&n.kind()))
            .map(|n| Range {
                start: ts_to_lsp_position(&n.start_position()),
                end: ts_to_lsp_position(&n.end_position()),
            })
    }

    pub fn rename(
        &self,
        uri: &Url,
        pos: &Position,
        new_text: &str,
        content: impl AsRef<[u8]>,
    ) -> Option<WorkspaceEdit> {
        let old_text = self
            .get_node_text_at_position(pos, content.as_ref())
            .unwrap_or_default();

        let mut changes = HashMap::new();

        let mut cursor = self.tree.root_node().walk();
        let diff: Vec<_> = Self::walk_and_collect_kinds(&mut cursor, &["identifier"])
            .into_iter()
            .filter(|n| n.utf8_text(content.as_ref()).unwrap() == old_text)
            .map(|n| TextEdit {
                new_text: new_text.to_string(),
                range: Range {
                    start: ts_to_lsp_position(&n.start_position()),
                    end: ts_to_lsp_position(&n.end_position()),
                },
            })
            .collect();

        if diff.is_empty() {
            return None;
        }

        changes.insert(uri.clone(), diff);

        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }

    pub fn collect_parse_errors(&self, uri: &Url) -> PublishDiagnosticsParams {
        let diagnostics = self
            .find_childrens_by_kinds(&["ERROR"])
            .into_iter()
            .map(|n| Diagnostic {
                range: Range {
                    start: ts_to_lsp_position(&n.start_position()),
                    end: ts_to_lsp_position(&n.end_position()),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("protols".to_string()),
                message: "Syntax error".to_string(),
                ..Default::default()
            })
            .collect();
        PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: None,
        }
    }
}

#[cfg(test)]
mod test {

    use async_lsp::lsp_types::{
        DiagnosticSeverity, DocumentSymbol, MarkedString, Position, Range, SymbolKind, TextEdit,
        Url,
    };

    use super::ProtoParser;

    #[test]
    fn test_find_children_by_kind() {
        let contents = r#"syntax = "proto3";

package com.book;

message Book {

    message Author {
        string name = 1;
        string country = 2;
    };
    // This is a multi line comment on the field name
    // Of a message called Book
    int64 isbn = 1;
    string title = 2;
    Author author = 3;
}
"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let nodes = tree.find_childrens_by_kinds(&["message_name"]);

        assert_eq!(nodes.len(), 2);

        let names: Vec<_> = nodes
            .into_iter()
            .map(|n| n.utf8_text(contents.as_ref()).unwrap())
            .collect();
        assert_eq!(names[0], "Book");
        assert_eq!(names[1], "Author");
    }

    #[test]
    fn test_collect_parse_error() {
        let url = "file://foo/bar.proto";
        let contents = r#"syntax = "proto3";

package test;

message Foo {
	reserved 1;
	reserved "baz";
	int bar = 2;
}"#;

        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let diagnostics = tree.collect_parse_errors(&url.parse().unwrap());
        assert_eq!(diagnostics.uri, Url::parse(url).unwrap());
        assert_eq!(diagnostics.diagnostics.len(), 0);

        let url = "file://foo/bar.proto";
        let contents = r#"syntax = "proto3";

package com.book;

message Book {
    message Author {
        string name;
        string country = 2;
    };
}"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let diagnostics = tree.collect_parse_errors(&url.parse().unwrap());

        assert_eq!(diagnostics.uri, Url::parse(url).unwrap());
        assert_eq!(diagnostics.diagnostics.len(), 1);

        let error = &diagnostics.diagnostics[0];
        assert_eq!(error.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(error.source, Some("protols".to_owned()));
        assert_eq!(error.message, "Syntax error");
        assert_eq!(
            error.range,
            Range {
                start: Position {
                    line: 6,
                    character: 8
                },
                end: Position {
                    line: 6,
                    character: 19
                }
            }
        );
    }

    #[test]
    fn test_rename() {
        let uri = "file://foo/bar.proto".parse().unwrap();
        let pos_book_rename = Position {
            line: 5,
            character: 9,
        };
        let pos_author_rename = Position {
            line: 21,
            character: 10,
        };
        let pos_non_renamble = Position {
            line: 24,
            character: 4,
        };
        let contents = r#"syntax = "proto3";

package com.book;

// A Book is book
message Book {

    // This is represents author
    // A author is a someone who writes books
    //
    // Author has a name and a country where they were born
    message Author {
        string name = 1;
        string country = 2;
    };
    Author author = 1;
    int price_usd = 2;
}

message Library {
    repeated Book books = 1;
    Book.Author collection = 2;
}

service Myservice {
    rpc GetBook(Empty) returns (Book);
}
"#;

        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        let res = tree.rename(&uri, &pos_book_rename, "Kitab", contents);
        assert!(res.is_some());
        let changes = res.unwrap().changes;
        assert!(changes.is_some());
        let changes = changes.unwrap();
        assert!(changes.contains_key(&uri));
        let edits = changes.get(&uri).unwrap();

        assert_eq!(
            *edits,
            vec![
                TextEdit {
                    range: Range {
                        start: Position {
                            line: 5,
                            character: 8,
                        },
                        end: Position {
                            line: 5,
                            character: 12,
                        },
                    },
                    new_text: "Kitab".to_string(),
                },
                TextEdit {
                    range: Range {
                        start: Position {
                            line: 20,
                            character: 13,
                        },
                        end: Position {
                            line: 20,
                            character: 17,
                        },
                    },
                    new_text: "Kitab".to_string(),
                },
                TextEdit {
                    range: Range {
                        start: Position {
                            line: 21,
                            character: 4,
                        },
                        end: Position {
                            line: 21,
                            character: 8,
                        },
                    },
                    new_text: "Kitab".to_string(),
                },
                TextEdit {
                    range: Range {
                        start: Position {
                            line: 25,
                            character: 32,
                        },
                        end: Position {
                            line: 25,
                            character: 36,
                        },
                    },
                    new_text: "Kitab".to_string(),
                },
            ],
        );

        let res = tree.rename(&uri, &pos_author_rename, "Writer", contents);
        assert!(res.is_some());
        let changes = res.unwrap().changes;
        assert!(changes.is_some());
        let changes = changes.unwrap();
        assert!(changes.contains_key(&uri));
        let edits = changes.get(&uri).unwrap();

        assert_eq!(
            *edits,
            vec![
                TextEdit {
                    range: Range {
                        start: Position {
                            line: 11,
                            character: 12,
                        },
                        end: Position {
                            line: 11,
                            character: 18,
                        },
                    },
                    new_text: "Writer".to_string(),
                },
                TextEdit {
                    range: Range {
                        start: Position {
                            line: 15,
                            character: 4,
                        },
                        end: Position {
                            line: 15,
                            character: 10,
                        },
                    },
                    new_text: "Writer".to_string(),
                },
                TextEdit {
                    range: Range {
                        start: Position {
                            line: 21,
                            character: 9,
                        },
                        end: Position {
                            line: 21,
                            character: 15,
                        },
                    },
                    new_text: "Writer".to_string(),
                },
            ],
        );

        let res = tree.rename(&uri, &pos_non_renamble, "Doesn't matter", contents);
        assert!(res.is_none());
    }

    #[test]
    fn test_can_rename() {
        let pos_rename = Position {
            line: 5,
            character: 9,
        };
        let pos_non_rename = Position {
            line: 2,
            character: 2,
        };
        let contents = r#"syntax = "proto3";

package com.book;

// A Book is book
message Book {

    // This is represents author
    // A author is a someone who writes books
    //
    // Author has a name and a country where they were born
    message Author {
        string name = 1;
        string country = 2;
    };
}
"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let res = tree.can_rename(&pos_rename);

        assert!(res.is_some());
        assert_eq!(
            res.unwrap(),
            Range {
                start: Position {
                    line: 5,
                    character: 8
                },
                end: Position {
                    line: 5,
                    character: 12
                },
            },
        );

        let res = tree.can_rename(&pos_non_rename);
        assert!(res.is_none());
    }

    #[test]
    fn test_hover() {
        let posbook = Position {
            line: 5,
            character: 9,
        };
        let posinvalid = Position {
            line: 0,
            character: 1,
        };
        let posauthor = Position {
            line: 11,
            character: 14,
        };
        let posts = Position {
            line: 14,
            character: 14,
        };
        let contents = r#"syntax = "proto3";

package com.book;

// A Book is book
message Book {

    // This is represents author
    // A author is a someone who writes books
    //
    // Author has a name and a country where they were born
    message Author {
        string name = 1;
        string country = 2;
        google.protobuf.Type ts = 3;
    };
}
"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let res = tree.hover(&posbook, contents);

        assert_eq!(res.len(), 1);
        assert_eq!(res[0], MarkedString::String("A Book is book".to_owned()));

        let res = tree.hover(&posinvalid, contents);
        assert_eq!(res.len(), 0);

        let res = tree.hover(&posauthor, contents);
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0],
            MarkedString::String(
                r#"This is represents author
A author is a someone who writes books

Author has a name and a country where they were born"#
                    .to_owned()
            )
        );

        let res = tree.hover(&posts, contents);
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0],
            MarkedString::String("A protocol buffer message type".to_owned())
        )
    }

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

    #[test]
    fn test_goto_definition() {
        let url = "file://foo/bar.proto";
        let posinvalid = Position {
            line: 0,
            character: 1,
        };
        let posauthor = Position {
            line: 10,
            character: 5,
        };
        let contents = r#"syntax = "proto3";

package com.book;

message Book {
    message Author {
        string name = 1;
        string country = 2;
    };

    Author author = 1;
    string isbn = 2;
}
"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let res = tree.definition(&posauthor, &url.parse().unwrap(), contents);

        assert_eq!(res.len(), 1);
        assert_eq!(res[0].uri, Url::parse(url).unwrap());
        assert_eq!(
            res[0].range,
            Range {
                start: Position {
                    line: 5,
                    character: 12
                },
                end: Position {
                    line: 5,
                    character: 18
                },
            }
        );

        let res = tree.definition(&posinvalid, &url.parse().unwrap(), contents);
        assert_eq!(res.len(), 0);
    }
}
