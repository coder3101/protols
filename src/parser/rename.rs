use async_lsp::lsp_types::{Location, Position, Range, TextEdit};
use tree_sitter::Node;

use crate::{nodekind::NodeKind, utils::ts_to_lsp_position};

use super::ParsedTree;

impl ParsedTree {
    pub fn can_rename(&self, pos: &Position) -> Option<Range> {
        self.get_node_at_position(pos)
            .filter(NodeKind::is_identifier)
            .and_then(|n| {
                if let Some(parent) = n.parent()
                    && NodeKind::is_renameable(&parent)
                {
                    Some(Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    })
                } else {
                    None
                }
            })
    }

    /// When the cursor is on a type-reference identifier (inside a
    /// `message_or_enum_type` node), return the partial qualified path up to
    /// and including the segment under cursor. This is the identifier whose
    /// declaration the rename should pivot to.
    ///
    /// For `Outer.Inner` with cursor on `Inner`, returns `Some("Outer.Inner")`.
    /// For `Outer.Inner` with cursor on `Outer`, returns `Some("Outer")`.
    /// For a non-reference position, returns `None`.
    ///
    /// The reconstruction walks every child of `message_or_enum_type` (named
    /// *and* anonymous) and concatenates their `utf8_text`. This depends on
    /// tree-sitter-proto emitting the `.` separators as anonymous children
    /// whose `utf8_text` is literally `"."`, and on the grammar leaving no
    /// whitespace between identifier tokens. Both invariants are exercised by
    /// `test_rename_pivot_identifier_qualified`, which asserts the exact
    /// `"Book.Author"` reconstruction — a grammar change that violated either
    /// invariant would fail that test rather than silently produce e.g.
    /// `"BookAuthor"`.
    pub fn rename_pivot_identifier(
        &self,
        pos: &Position,
        content: impl AsRef<[u8]>,
    ) -> Option<String> {
        let n = self.get_node_at_position(pos)?;
        if !NodeKind::is_identifier(&n) {
            return None;
        }
        let parent = n.parent()?;
        if !NodeKind::is_field_name(&parent) {
            return None;
        }

        let cursor_end = n.end_byte();
        let bytes = content.as_ref();
        let mut path = String::new();
        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            let text = child.utf8_text(bytes).ok()?;
            path.push_str(text);
            if child.end_byte() >= cursor_end {
                break;
            }
        }
        Some(path)
    }

    /// If the given position is on the rpc name of an rpc declaration, returns
    /// the rpc's name along with its declared request and response type texts.
    /// Used to drive the rpc/request/response chained rename.
    pub fn rpc_at_position(
        &self,
        pos: &Position,
        content: impl AsRef<[u8]>,
    ) -> Option<(String, String, String)> {
        let n = self.get_node_at_position(pos)?;
        if !NodeKind::is_identifier(&n) {
            return None;
        }
        let parent = n.parent()?;
        if parent.kind() != NodeKind::RpcName.as_str() {
            return None;
        }
        let rpc = parent.parent()?;
        let bytes = content.as_ref();
        let mut cursor = rpc.walk();
        let mut types = rpc
            .children(&mut cursor)
            .filter(|c| c.kind() == NodeKind::FieldName.as_str());
        let request = types.next()?.utf8_text(bytes).ok()?.to_owned();
        let response = types.next()?.utf8_text(bytes).ok()?.to_owned();
        let rpc_name = n.utf8_text(bytes).ok()?.to_owned();
        Some((rpc_name, request, response))
    }

    /// If the given position is on a message name, returns that name.
    pub fn message_name_at_position(
        &self,
        pos: &Position,
        content: impl AsRef<[u8]>,
    ) -> Option<String> {
        let n = self.get_node_at_position(pos)?;
        if !NodeKind::is_identifier(&n) {
            return None;
        }
        let parent = n.parent()?;
        if parent.kind() != NodeKind::MessageName.as_str() {
            return None;
        }
        Some(n.utf8_text(content.as_ref()).ok()?.to_owned())
    }

    /// Returns the (request, response) type texts for every `rpc` node in this
    /// tree. Used to verify that a request/response type is uniquely used by a
    /// single rpc before chain-renaming it.
    pub fn all_rpc_signatures(&self, content: impl AsRef<[u8]>) -> Vec<(String, String)> {
        let bytes = content.as_ref();
        let mut out = vec![];
        for rpc in self.find_all_nodes(|n: &Node| n.kind() == "rpc") {
            let mut cursor = rpc.walk();
            let mut types = rpc
                .children(&mut cursor)
                .filter(|c| c.kind() == NodeKind::FieldName.as_str());
            let Some(req) = types.next().and_then(|c| c.utf8_text(bytes).ok()) else {
                continue;
            };
            let Some(resp) = types.next().and_then(|c| c.utf8_text(bytes).ok()) else {
                continue;
            };
            out.push((req.to_owned(), resp.to_owned()));
        }
        out
    }

    fn nodes_within<'a>(
        &self,
        n: Node<'a>,
        identifier: &str,
        content: impl AsRef<[u8]>,
    ) -> Option<Vec<Node<'a>>> {
        n.parent().map(|p| {
            self.find_all_nodes_from(p, NodeKind::is_field_name)
                .into_iter()
                .filter(|i| i.utf8_text(content.as_ref()).expect("utf-8 parse error") == identifier)
                .collect()
        })
    }

    pub fn reference_tree(
        &self,
        pos: &Position,
        content: impl AsRef<[u8]>,
    ) -> Option<(Vec<Location>, String)> {
        let rename_range = self.can_rename(pos)?;

        let mut res = vec![Location {
            uri: self.uri.clone(),
            range: rename_range,
        }];

        let nodes = self.get_ancestor_nodes_at_position(pos);
        let mut i = 1;
        let mut otext = nodes.first()?.utf8_text(content.as_ref()).ok()?.to_owned();
        while nodes.len() > i {
            let id = nodes[i].utf8_text(content.as_ref()).ok()?;
            if let Some(inodes) = self.nodes_within(nodes[i], &otext, content.as_ref()) {
                res.extend(inodes.into_iter().map(|n| Location {
                    uri: self.uri.clone(),
                    range: Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    },
                }))
            }
            otext = format!("{id}.{otext}");
            i += 1
        }
        Some((res, otext))
    }

    pub fn rename_tree(
        &self,
        pos: &Position,
        new_name: &str,
        content: impl AsRef<[u8]>,
    ) -> Option<(Vec<TextEdit>, String, String)> {
        let rename_range = self.can_rename(pos)?;

        let mut v = vec![TextEdit {
            range: rename_range,
            new_text: new_name.to_owned(),
        }];

        let nodes = self.get_ancestor_nodes_at_position(pos);

        // Renameable symbols with no message ancestor: top-level enums, services, RPCs,
        // and the various field-like declarations (regular fields, map fields, oneof,
        // oneof fields, enum values). Only top-level enums are referenced as types
        // from other files; the rest are single-site, so we hand the workspace pass
        // a name it won't find — making it a harmless no-op without risking that a
        // field named the same as some lowercase type triggers an unwanted rename.
        if nodes.is_empty() {
            let n = self.get_node_at_position(pos)?;
            let identifier = n.utf8_text(content.as_ref()).ok()?.to_owned();
            let is_type_symbol = n
                .parent()
                .is_some_and(|p| p.kind() == NodeKind::EnumName.as_str());
            let (otext, ntext) = if is_type_symbol {
                (identifier, new_name.to_owned())
            } else {
                (new_name.to_owned(), new_name.to_owned())
            };
            return Some((v, otext, ntext));
        }

        let mut i = 1;
        let mut otext = nodes.first()?.utf8_text(content.as_ref()).ok()?.to_owned();
        let mut ntext = new_name.to_owned();

        while nodes.len() > i {
            let id = nodes[i].utf8_text(content.as_ref()).ok()?;

            if let Some(inodes) = self.nodes_within(nodes[i], &otext, content.as_ref()) {
                v.extend(inodes.into_iter().map(|n| TextEdit {
                    range: Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    },
                    new_text: ntext.to_owned(),
                }));
            }

            otext = format!("{id}.{otext}");
            ntext = format!("{id}.{ntext}");

            i += 1
        }

        Some((v, otext, ntext))
    }

    pub fn rename_field(
        &self,
        old_identifier: &str,
        new_identifier: &str,
        content: impl AsRef<[u8]>,
    ) -> Vec<TextEdit> {
        self.find_all_nodes(NodeKind::is_field_name)
            .into_iter()
            .filter(|n| {
                let ntext = n.utf8_text(content.as_ref()).expect("utf-8 parse error");
                let sc = format!("{old_identifier}.");
                ntext == old_identifier || ntext.starts_with(&sc)
            })
            .map(|n| {
                let text = n.utf8_text(content.as_ref()).expect("utf-8 parse error");
                TextEdit {
                    new_text: text.replace(old_identifier, new_identifier),
                    range: Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    },
                }
            })
            .collect()
    }

    pub fn reference_field(&self, id: &str, content: impl AsRef<[u8]>) -> Vec<Location> {
        self.find_all_nodes(NodeKind::is_field_name)
            .into_iter()
            .filter(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error") == id)
            .map(|n| Location {
                uri: self.uri.clone(),
                range: Range {
                    start: ts_to_lsp_position(&n.start_position()),
                    end: ts_to_lsp_position(&n.end_position()),
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Position, Url};
    use insta::assert_yaml_snapshot;

    use crate::parser::ProtoParser;

    #[test]
    fn test_rename() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let pos_book = Position {
            line: 5,
            character: 9,
        };
        let pos_author = Position {
            line: 11,
            character: 14,
        };
        let pos_non_rename = Position {
            line: 21,
            character: 5,
        };
        let contents = include_str!("input/test_rename.proto");

        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        let rename_fn = |nt: &str, pos: &Position| match tree.rename_tree(pos, nt, contents) {
            Some(k) => {
                let mut v = tree.rename_field(&k.1, &k.2, contents);
                v.extend(k.0);
                v
            }
            _ => {
                vec![]
            }
        };

        assert_yaml_snapshot!(rename_fn("Kitab", &pos_book));
        assert_yaml_snapshot!(rename_fn("Writer", &pos_author));
        assert_yaml_snapshot!(rename_fn("xyx", &pos_non_rename));
    }

    #[test]
    fn test_reference() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let pos_book = Position {
            line: 5,
            character: 9,
        };
        let pos_author = Position {
            line: 11,
            character: 14,
        };
        let pos_non_ref = Position {
            line: 21,
            character: 5,
        };
        let contents = include_str!("input/test_reference.proto");

        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        let reference_fn = |pos: &Position| match tree.reference_tree(pos, contents) {
            Some(k) => {
                let mut v = tree.reference_field(&k.1, contents);
                v.extend(k.0);
                v
            }
            _ => {
                vec![]
            }
        };

        assert_yaml_snapshot!(reference_fn(&pos_book));
        assert_yaml_snapshot!(reference_fn(&pos_author));
        assert_yaml_snapshot!(reference_fn(&pos_non_ref));
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
        let pos_inner_type = Position {
            line: 19,
            character: 11,
        };
        let pos_outer_type = Position {
            line: 19,
            character: 5,
        };

        let contents = include_str!("input/test_can_rename.proto");
        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());

        let tree = parsed.unwrap();
        assert_yaml_snapshot!(tree.can_rename(&pos_rename));
        assert_yaml_snapshot!(tree.can_rename(&pos_non_rename));
        assert_yaml_snapshot!(tree.can_rename(&pos_inner_type));
        assert_yaml_snapshot!(tree.can_rename(&pos_outer_type));
    }

    #[test]
    fn test_can_rename_service_and_rpc() {
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new().parse(uri, contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        let pos_service = Position {
            line: 10,
            character: 10,
        };
        let pos_rpc = Position {
            line: 11,
            character: 9,
        };
        let pos_rpc_request_type = Position {
            line: 11,
            character: 17,
        };

        assert_yaml_snapshot!(tree.can_rename(&pos_service));
        assert_yaml_snapshot!(tree.can_rename(&pos_rpc));
        // Type references inside an RPC declaration are renameable from the
        // reference site; the LSP layer pivots to the declaration.
        assert_yaml_snapshot!(tree.can_rename(&pos_rpc_request_type));
    }

    #[test]
    fn test_rename_service_and_rpc() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new().parse(uri, contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        let pos_service = Position {
            line: 10,
            character: 10,
        };
        let pos_rpc = Position {
            line: 11,
            character: 9,
        };

        let rename_fn = |nt: &str, pos: &Position| match tree.rename_tree(pos, nt, contents) {
            Some(k) => {
                let mut v = tree.rename_field(&k.1, &k.2, contents);
                v.extend(k.0);
                v
            }
            _ => vec![],
        };

        assert_yaml_snapshot!(rename_fn("Catalog", &pos_service));
        assert_yaml_snapshot!(rename_fn("FetchBook", &pos_rpc));
    }

    #[test]
    fn test_rename_pivot_identifier() {
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new().parse(uri, contents).unwrap();

        // `Empty` reference at line 11 (the rpc Get(Empty) ...): char 16 = 'E'
        let pos_unqualified_ref = Position {
            line: 11,
            character: 17,
        };
        // `Book` return type at line 11 char 32..36
        let pos_other_ref = Position {
            line: 11,
            character: 33,
        };
        // Service declaration site — not a reference, so no pivot needed
        let pos_decl = Position {
            line: 10,
            character: 10,
        };

        assert_eq!(
            parsed.rename_pivot_identifier(&pos_unqualified_ref, contents),
            Some("Empty".to_owned())
        );
        assert_eq!(
            parsed.rename_pivot_identifier(&pos_other_ref, contents),
            Some("Book".to_owned())
        );
        assert_eq!(parsed.rename_pivot_identifier(&pos_decl, contents), None);
    }

    #[test]
    fn test_rpc_at_position_and_signatures() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new().parse(uri, contents).unwrap();

        // `GetBook` rpc at line 11 chars 8..15
        let pos_rpc = Position {
            line: 11,
            character: 10,
        };
        assert_eq!(
            parsed.rpc_at_position(&pos_rpc, contents),
            Some(("GetBook".to_owned(), "Empty".to_owned(), "Book".to_owned(),)),
        );

        // Cursor on a non-rpc identifier should return None.
        let pos_service = Position {
            line: 10,
            character: 10,
        };
        assert_eq!(parsed.rpc_at_position(&pos_service, contents), None);

        // all_rpc_signatures should pick up both rpcs in the file.
        let sigs = parsed.all_rpc_signatures(contents);
        assert_eq!(
            sigs,
            vec![
                ("Empty".to_owned(), "Book".to_owned()),
                ("Empty".to_owned(), "Book".to_owned()),
            ]
        );
    }

    #[test]
    fn test_message_name_at_position() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new().parse(uri, contents).unwrap();

        // `Book` declaration at line 6 chars 8..12
        let pos = Position {
            line: 6,
            character: 9,
        };
        assert_eq!(
            parsed.message_name_at_position(&pos, contents),
            Some("Book".to_owned())
        );

        // RPC name shouldn't match.
        let pos_rpc = Position {
            line: 11,
            character: 10,
        };
        assert_eq!(parsed.message_name_at_position(&pos_rpc, contents), None);
    }

    #[test]
    fn test_rename_field_and_enum_value() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_field.proto");
        let parsed = ProtoParser::new().parse(uri, contents).unwrap();
        let tree = parsed;

        let rename_fn = |nt: &str, pos: &Position| match tree.rename_tree(pos, nt, contents) {
            Some(k) => {
                let mut v = tree.rename_field(&k.1, &k.2, contents);
                v.extend(k.0);
                v
            }
            _ => vec![],
        };

        // Enum value: RED at line 5 chars 4..7
        let pos_enum_value = Position {
            line: 5,
            character: 5,
        };
        // Plain field: title at line 14 chars 11..16
        let pos_plain_field = Position {
            line: 14,
            character: 12,
        };
        // User-type field: author at line 15 chars 11..17
        let pos_user_type_field = Position {
            line: 15,
            character: 12,
        };
        // Map field: counts at line 16 chars 23..29
        let pos_map_field = Position {
            line: 16,
            character: 24,
        };
        // Oneof name: body at line 17 chars 10..14
        let pos_oneof_name = Position {
            line: 17,
            character: 11,
        };
        // Oneof field: text at line 18 chars 15..19
        let pos_oneof_field = Position {
            line: 18,
            character: 16,
        };

        assert_yaml_snapshot!(rename_fn("CRIMSON", &pos_enum_value));
        assert_yaml_snapshot!(rename_fn("name", &pos_plain_field));
        assert_yaml_snapshot!(rename_fn("writer", &pos_user_type_field));
        assert_yaml_snapshot!(rename_fn("tallies", &pos_map_field));
        assert_yaml_snapshot!(rename_fn("content", &pos_oneof_name));
        assert_yaml_snapshot!(rename_fn("words", &pos_oneof_field));
    }

    #[test]
    fn test_can_rename_field_and_enum_value() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_field.proto");
        let parsed = ProtoParser::new().parse(uri, contents).unwrap();

        // Cursor on a type identifier inside a field (`Author` at line 15
        // chars 4..10) is a reference site — already supported.
        let pos_type_ref = Position {
            line: 15,
            character: 6,
        };
        // Cursor on the field name should now be renameable.
        let pos_plain_field = Position {
            line: 14,
            character: 12,
        };
        // Cursor on the int_lit `1` is not a renameable identifier.
        let pos_field_number = Position {
            line: 14,
            character: 19,
        };

        assert_yaml_snapshot!(parsed.can_rename(&pos_type_ref));
        assert_yaml_snapshot!(parsed.can_rename(&pos_plain_field));
        assert_yaml_snapshot!(parsed.can_rename(&pos_field_number));
    }

    #[test]
    fn test_rename_pivot_identifier_qualified() {
        // `Book.Author a = 1;` at line 19 of test_can_rename.proto
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let contents = include_str!("input/test_can_rename.proto");
        let parsed = ProtoParser::new().parse(uri, contents).unwrap();

        // Cursor on `Book` (the outer segment): chars 4..8
        let pos_outer = Position {
            line: 19,
            character: 5,
        };
        // Cursor on `Author` (the inner segment): chars 9..15
        let pos_inner = Position {
            line: 19,
            character: 11,
        };

        assert_eq!(
            parsed.rename_pivot_identifier(&pos_outer, contents),
            Some("Book".to_owned())
        );
        assert_eq!(
            parsed.rename_pivot_identifier(&pos_inner, contents),
            Some("Book.Author".to_owned())
        );
    }
}
