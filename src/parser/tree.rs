use async_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, TreeCursor};

use crate::{
    context::{hoverable::Hoverables, jumpable::Jumpable},
    nodekind::NodeKind,
    utils::{lsp_to_ts_point, ts_to_lsp_position},
};

use super::ParsedTree;

impl ParsedTree {
    pub(super) fn walk_and_filter<'a>(
        cursor: &mut TreeCursor<'a>,
        f: fn(&Node) -> bool,
        early: bool,
    ) -> Vec<Node<'a>> {
        let mut v = vec![];

        loop {
            let node = cursor.node();

            if f(&node) {
                v.push(node);
                if early {
                    break;
                }
            }

            if cursor.goto_first_child() {
                v.extend(Self::walk_and_filter(cursor, f, early));
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

    pub fn get_user_defined_text<'a>(
        &'a self,
        pos: &Position,
        content: &'a [u8],
    ) -> Option<&'a str> {
        self.get_user_defined_node(pos)
            .map(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error"))
    }

    pub fn get_jumpable_at_position(&self, pos: &Position, content: &[u8]) -> Option<Jumpable> {
        let n = self.get_node_at_position(pos)?;

        // If node is import path. return the whole path, removing the quotes
        if n.parent().filter(NodeKind::is_import_path).is_some() {
            return Some(Jumpable::Import(
                n.utf8_text(content)
                    .expect("utf-8 parse error")
                    .trim_matches('"')
                    .to_string(),
            ));
        }

        // If node is user defined enum/message
        if let Some(identifier) = self.get_user_defined_text(pos, content) {
            return Some(Jumpable::Identifier(identifier.to_string()));
        }

        None
    }

    pub fn get_hoverable_at_position<'a>(
        &'a self,
        pos: &Position,
        content: &'a [u8],
    ) -> Option<Hoverables> {
        let n = self.get_node_at_position(pos)?;

        // If node is import path. return the whole path, removing the quotes
        if n.parent().filter(NodeKind::is_import_path).is_some() {
            return Some(Hoverables::ImportPath(
                n.utf8_text(content)
                    .expect("utf-8 parse error")
                    .trim_matches('"')
                    .to_string(),
            ));
        }

        // If node is user defined enum/message
        if let Some(identifier) = self.get_user_defined_text(pos, content) {
            return Some(Hoverables::Identifier(identifier.to_string()));
        }

        // Lastly; fallback to either wellknown or builtin types
        Some(Hoverables::FieldType(n.kind().to_string()))
    }

    pub fn get_ancestor_nodes_at_position<'a>(&'a self, pos: &Position) -> Vec<Node<'a>> {
        let Some(mut n) = self.get_user_defined_node(pos) else {
            return vec![];
        };

        let mut nodes = vec![];
        while let Some(p) = n.parent() {
            if NodeKind::is_message(&p) {
                for i in 0..p.child_count() {
                    let t = p.child(i as u32).unwrap();
                    if NodeKind::is_message_name(&t) {
                        nodes.push(t);
                    }
                }
            }
            n = p;
        }
        nodes
    }

    pub fn get_user_defined_node<'a>(&'a self, pos: &Position) -> Option<Node<'a>> {
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

    pub fn get_node_at_position<'a>(&'a self, pos: &Position) -> Option<Node<'a>> {
        let pos = lsp_to_ts_point(pos);
        self.tree.root_node().descendant_for_point_range(pos, pos)
    }

    pub fn find_all_nodes(&self, f: fn(&Node) -> bool) -> Vec<Node<'_>> {
        self.find_all_nodes_from(self.tree.root_node(), f)
    }

    pub fn find_all_nodes_from<'a>(&self, n: Node<'a>, f: fn(&Node) -> bool) -> Vec<Node<'a>> {
        let mut cursor = n.walk();
        Self::walk_and_filter(&mut cursor, f, false)
    }

    pub fn find_first_node(&self, f: fn(&Node) -> bool) -> Vec<Node<'_>> {
        self.find_node_from(self.tree.root_node(), f)
    }

    pub fn find_node_from<'a>(&self, n: Node<'a>, f: fn(&Node) -> bool) -> Vec<Node<'a>> {
        let mut cursor = n.walk();
        Self::walk_and_filter(&mut cursor, f, true)
    }

    pub fn get_package_name<'a>(&self, content: &'a [u8]) -> Option<&'a str> {
        self.find_first_node(NodeKind::is_package_name)
            .first()
            .map(|n| n.utf8_text(content).expect("utf-8 parse error"))
    }

    pub fn get_import_node(&self) -> Vec<Node<'_>> {
        self.find_all_nodes(NodeKind::is_import_path)
            .into_iter()
            .filter_map(|n| n.child_by_field_name("path"))
            .collect()
    }

    pub fn get_import_paths<'a>(&self, content: &'a [u8]) -> Vec<&'a str> {
        self.get_import_node()
            .into_iter()
            .map(|n| {
                n.utf8_text(content)
                    .expect("utf-8 parse error")
                    .trim_matches('"')
            })
            .collect()
    }

    pub fn get_import_path_range(&self, content: &[u8], import: Vec<String>) -> Vec<Range> {
        self.get_import_node()
            .into_iter()
            .filter(|n| {
                let t = n
                    .utf8_text(content)
                    .expect("utf8-parse error")
                    .trim_matches('"');
                import.iter().any(|i| i == t)
            })
            .map(|n| Range {
                start: ts_to_lsp_position(&n.start_position()),
                end: ts_to_lsp_position(&n.end_position()),
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use insta::assert_yaml_snapshot;

    use crate::{nodekind::NodeKind, parser::ProtoParser};

    #[test]
    fn test_filter() {
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let contents = include_str!("input/test_filter.proto");
        let parsed = ProtoParser::new().parse(uri, contents);

        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let nodes = tree.find_all_nodes(NodeKind::is_message_name);

        assert_eq!(nodes.len(), 2);

        let names: Vec<_> = nodes
            .into_iter()
            .map(|n| n.utf8_text(contents.as_ref()).unwrap())
            .collect();

        assert_yaml_snapshot!(names);

        let package_name = tree.get_package_name(contents.as_ref());
        assert_yaml_snapshot!(package_name);
        let imports = tree.get_import_paths(contents.as_ref());
        assert_yaml_snapshot!(imports);
    }
}
