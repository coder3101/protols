use async_lsp::lsp_types::Position;
use tree_sitter::{Node, TreeCursor};

use crate::utils::lsp_to_ts_point;

use super::{nodekind::NodeKind, ParsedTree};

impl ParsedTree {
    pub(super) fn walk_and_collect_filter<'a>(
        cursor: &mut TreeCursor<'a>,
        f: fn(&Node) -> bool,
    ) -> Vec<Node<'a>> {
        let mut v = vec![];

        loop {
            let node = cursor.node();

            if f(&node) {
                v.push(node)
            }

            if cursor.goto_first_child() {
                v.extend(Self::walk_and_collect_filter(cursor, f));
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        v
    }

    pub(super) fn advance_cursor_to(cursor: &mut TreeCursor<'_>, nid: usize) -> bool {
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

    pub(super) fn get_node_text_at_position<'a>(
        &'a self,
        pos: &Position,
        content: &'a [u8],
    ) -> Option<&'a str> {
        self.get_node_at_position(pos)
            .map(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error"))
    }

    pub(super) fn get_actionable_node_text_at_position<'a>(
        &'a self,
        pos: &Position,
        content: &'a [u8],
    ) -> Option<&'a str> {
        self.get_actionable_node_at_position(pos)
            .map(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error"))
    }

    pub(super) fn get_actionable_node_at_position<'a>(
        &'a self,
        pos: &Position,
    ) -> Option<Node<'a>> {
        self.get_node_at_position(pos)
            .map(|n| {
                if NodeKind::is_actionable(&n) {
                    n
                } else {
                    n.parent().unwrap()
                }
            })
            .filter(NodeKind::is_actionable)
    }

    pub(super) fn get_node_at_position<'a>(&'a self, pos: &Position) -> Option<Node<'a>> {
        let pos = lsp_to_ts_point(pos);
        self.tree.root_node().descendant_for_point_range(pos, pos)
    }

    pub(super) fn filter_node(&self, f: fn(&Node) -> bool) -> Vec<Node> {
        let mut cursor = self.tree.root_node().walk();
        Self::walk_and_collect_filter(&mut cursor, f)
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use tree_sitter::Node;

    use crate::parser::ProtoParser;

    fn is_message(n: &Node) -> bool {
        n.kind() == "message_name"
    }

    #[test]
    fn test_find_children_by_kind() {
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
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
        let parsed = ProtoParser::new().parse(uri, contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let nodes = tree.filter_node(is_message);

        assert_eq!(nodes.len(), 2);

        let names: Vec<_> = nodes
            .into_iter()
            .map(|n| n.utf8_text(contents.as_ref()).unwrap())
            .collect();
        assert_eq!(names[0], "Book");
        assert_eq!(names[1], "Author");
    }
}
