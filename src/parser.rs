use async_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, Location, MarkedString, Position, PublishDiagnosticsParams,
    Range, Url,
};
use tracing::info;
use tree_sitter::{Node, Tree, TreeCursor};

use crate::utils::{lsp_to_ts_point, ts_to_lsp_position};

pub struct ProtoParser {
    parser: tree_sitter::Parser,
}

pub struct ParsedTree {
    tree: Tree,
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
    fn walk_and_collect_kinds<'a>(
        &self,
        cursor: &mut TreeCursor<'a>,
        kinds: &[&str],
    ) -> Vec<Node<'a>> {
        let mut v = vec![];

        loop {
            let node = cursor.node();

            if kinds.contains(&node.kind()) {
                v.push(node)
            }

            if cursor.goto_first_child() {
                v.extend(self.walk_and_collect_kinds(cursor, kinds));
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        v
    }

    fn advance_cursor_to<'a>(&self, cursor: &mut TreeCursor<'a>, nid: usize) -> bool {
        loop {
            let node = cursor.node();
            if node.id() == nid {
                return true;
            }
            if cursor.goto_first_child() {
                if self.advance_cursor_to(cursor, nid) {
                    return true;
                }
                cursor.goto_parent();
            }
            if !cursor.goto_next_sibling() {
                return false;
            }
        }
    }

    fn find_preceeding_comments(&self, nid: usize, content: impl AsRef<[u8]>) -> Option<String> {
        let root = self.tree.root_node();
        let mut cursor = root.walk();

        info!("Looking for node with id: {nid}");

        self.advance_cursor_to(&mut cursor, nid);
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
        return if comments.len() != 0 {
            comments.reverse();
            Some(comments.join("\n"))
        } else {
            None
        };
    }
}

impl ParsedTree {
    pub fn get_node_text_at_position<'a>(
        &'a self,
        pos: &Position,
        content: &'a [u8],
    ) -> Option<&'a str> {
        let pos = lsp_to_ts_point(pos);
        self.tree
            .root_node()
            .descendant_for_point_range(pos, pos)
            .map(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error"))
    }

    pub fn find_childrens_by_kinds(&self, kinds: &[&str]) -> Vec<Node> {
        let mut cursor = self.tree.root_node().walk();
        self.walk_and_collect_kinds(&mut cursor, kinds)
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
                .find_childrens_by_kinds(&["message_name", "enum_name"])
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
        let text = self.get_node_text_at_position(pos, content.as_ref());
        info!("Looking for hover response on: {:?}", text);
        match text {
            Some(text) => self
                .find_childrens_by_kinds(&["message_name", "enum_name", "service_name", "rpc_name"])
                .into_iter()
                .filter(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error") == text)
                .filter_map(|n| self.find_preceeding_comments(n.id(), content.as_ref()))
                .map(|s| MarkedString::String(s))
                .collect(),
            None => vec![],
        }
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
    use async_lsp::lsp_types::{DiagnosticSeverity, MarkedString, Position, Range, Url};

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

package com.book;

message Book {
    message Author {
        string name;
        string country = 2;
    };
}
"#;
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
