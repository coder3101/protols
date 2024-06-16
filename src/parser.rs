use async_lsp::lsp_types::{Location, MarkedString, Position, Range, Url};
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
        if let Err(e) = parser.set_language(&tree_sitter_proto::language()) {
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
}
