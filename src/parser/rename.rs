use std::collections::HashMap;

use async_lsp::lsp_types::{Position, Range, TextEdit, WorkspaceEdit};

use crate::utils::ts_to_lsp_position;

use super::{nodekind::NodeKind, ParsedTree};

impl ParsedTree {
    pub fn can_rename(&self, pos: &Position) -> Option<Range> {
        self.get_node_at_position(pos)
            .filter(NodeKind::is_identifier)
            .map(|n| n.parent().unwrap()) // Safety: Identifier must have a parent node
            .filter(NodeKind::is_actionable)
            .map(|n| Range {
                start: ts_to_lsp_position(&n.start_position()),
                end: ts_to_lsp_position(&n.end_position()),
            })
    }

    pub fn rename(
        &self,
        pos: &Position,
        new_text: &str,
        content: impl AsRef<[u8]>,
    ) -> Option<WorkspaceEdit> {
        let old_text = self
            .get_node_text_at_position(pos, content.as_ref())
            .unwrap_or_default();

        let mut changes = HashMap::new();

        let diff: Vec<_> = self
            .filter_nodes(NodeKind::is_identifier)
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

        changes.insert(self.uri.clone(), diff);

        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Position, Range, TextEdit, Url};

    use crate::parser::ProtoParser;

    #[test]
    fn test_rename() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
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

        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        let res = tree.rename(&pos_book_rename, "Kitab", contents);
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

        let res = tree.rename(&pos_author_rename, "Writer", contents);
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

        let res = tree.rename(&pos_non_renamble, "Doesn't matter", contents);
        assert!(res.is_none());
    }

    #[test]
    fn test_can_rename() {
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
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
        let parsed = ProtoParser::new().parse(uri.clone(), contents);
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
}
