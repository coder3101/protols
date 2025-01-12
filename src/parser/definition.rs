use async_lsp::lsp_types::{Location, Range};
use tree_sitter::Node;

use crate::{nodekind::NodeKind, utils::ts_to_lsp_position};

use super::ParsedTree;

impl ParsedTree {
    pub fn definition(&self, identifier: &str, content: impl AsRef<[u8]>) -> Vec<Location> {
        let mut results = vec![];
        self.definition_impl(identifier, self.tree.root_node(), &mut results, content);
        results
    }
    fn definition_impl(
        &self,
        identifier: &str,
        n: Node,
        v: &mut Vec<Location>,
        content: impl AsRef<[u8]>,
    ) {
        if identifier.is_empty() {
            return;
        }

        match identifier.split_once('.') {
            Some((parent_identifier, remaining)) => {
                let child_node = self
                    .find_all_nodes_from(n, NodeKind::is_userdefined)
                    .into_iter()
                    .find(|n| {
                        n.utf8_text(content.as_ref()).expect("utf8-parse error")
                            == parent_identifier
                    })
                    .and_then(|n| n.parent());

                if let Some(inner) = child_node {
                    self.definition_impl(remaining, inner, v, content);
                }
            }
            None => {
                let locations: Vec<Location> = self
                    .find_all_nodes_from(n, NodeKind::is_userdefined)
                    .into_iter()
                    .filter(|n| {
                        n.utf8_text(content.as_ref()).expect("utf-8 parse error") == identifier
                    })
                    .map(|n| Location {
                        uri: self.uri.clone(),
                        range: Range {
                            start: ts_to_lsp_position(&n.start_position()),
                            end: ts_to_lsp_position(&n.end_position()),
                        },
                    })
                    .collect();

                v.extend(locations);
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
    fn test_goto_definition() {
        let url: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_goto_definition.proto");
        let parsed = ProtoParser::new().parse(url, contents);

        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        assert_yaml_snapshot!(tree.definition("Author", contents));
        assert_yaml_snapshot!(tree.definition("", contents));
    }
}
