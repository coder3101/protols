use async_lsp::lsp_types::{Location, Position, Range, TextEdit};
use tree_sitter::Node;

use crate::{nodekind::NodeKind, utils::ts_to_lsp_position};

use super::ParsedTree;

impl ParsedTree {
    pub fn can_rename(&self, pos: &Position) -> Option<Range> {
        self.get_node_at_position(pos)
            .filter(NodeKind::is_identifier)
            .and_then(|n| {
                if n.parent().is_some() && NodeKind::is_userdefined(&n.parent().unwrap()) {
                    Some(Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    })
                } else {
                    None
                }
            })
    }

    fn nodes_within<'a>(
        &self,
        n: Node<'a>,
        identifier: &str,
        content: impl AsRef<[u8]>,
    ) -> Option<Vec<Node<'a>>> {
        n.parent().map(|p| {
            self.filter_nodes_from(p, NodeKind::is_field_name)
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
        self.filter_nodes(NodeKind::is_field_name)
            .into_iter()
            .filter(|n| {
                let ntext = n.utf8_text(content.as_ref()).expect("utf-8 parse error");
                let sc = format!("{old_identifier}.");
                return ntext == old_identifier || ntext.starts_with(&sc);
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
        self.filter_nodes(NodeKind::is_field_name)
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

        let rename_fn = |nt: &str, pos: &Position| {
            if let Some(k) = tree.rename_tree(pos, nt, contents) {
                let mut v = tree.rename_field(&k.1, &k.2, contents);
                v.extend(k.0);
                v
            } else {
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

        let reference_fn = |pos: &Position| {
            if let Some(k) = tree.reference_tree(pos, contents) {
                let mut v = tree.reference_field(&k.1, contents);
                v.extend(k.0);
                v
            } else {
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
}
