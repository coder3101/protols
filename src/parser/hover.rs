use async_lsp::lsp_types::MarkedString;
use tree_sitter::Node;

use crate::nodekind::NodeKind;

use super::ParsedTree;

impl ParsedTree {
    pub(super) fn find_preceding_comments(
        &self,
        nid: usize,
        content: impl AsRef<[u8]>,
    ) -> Option<String> {
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

    pub fn hover(&self, identifier: &str, content: impl AsRef<[u8]>) -> Vec<String> {
        let mut results = vec![];
        self.hover_impl(identifier, self.tree.root_node(), &mut results, content);
        results
    }

    fn hover_impl(
        &self,
        identifier: &str,
        n: Node,
        v: &mut Vec<String>,
        content: impl AsRef<[u8]>,
    ) {
        if identifier.is_empty() {
            return;
        }

        match identifier.split_once('.') {
            Some((parent, child)) => {
                let child_node = self
                    .filter_nodes_from(n, NodeKind::is_userdefined)
                    .into_iter()
                    .find(|n| n.utf8_text(content.as_ref()).expect("utf8-parse error") == parent)
                    .and_then(|n| n.parent());

                if let Some(inner) = child_node {
                    self.hover_impl(child, inner, v, content);
                }
            }
            None => {
                let comments: Vec<String> = self
                    .filter_nodes_from(n, NodeKind::is_userdefined)
                    .into_iter()
                    .filter(|n| {
                        n.utf8_text(content.as_ref()).expect("utf-8 parse error") == identifier
                    })
                    .filter_map(|n| self.find_preceding_comments(n.id(), content.as_ref()))
                    .collect();

                v.extend(comments);
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
    fn test_hover() {
        let uri: Url = "file://foo.bar/p.proto".parse().unwrap();
        let contents = include_str!("input/test_hover.proto");
        let parsed = ProtoParser::new().parse(uri.clone(), contents);

        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        let res = tree.hover("Book", contents);
        assert_yaml_snapshot!(res);

        let res = tree.hover("", contents);
        assert_yaml_snapshot!(res);

        let res = tree.hover("Book.Author", contents);
        assert_yaml_snapshot!(res);

        let res = tree.hover("Comic.Author", contents);
        assert_yaml_snapshot!(res);

        let res = tree.hover("Author", contents);
        assert_yaml_snapshot!(res);
    }
}
