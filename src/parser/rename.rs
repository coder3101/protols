use std::collections::HashMap;

use async_lsp::lsp_types::{Position, Range, TextEdit, WorkspaceEdit};

use crate::utils::ts_to_lsp_position;

use super::{nodekind::NodeKind, ParsedTree};

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
    use async_lsp::lsp_types::{Position, Url};
    use insta::assert_yaml_snapshot;

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
        let contents = include_str!("input/test_rename.proto");

        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        assert_yaml_snapshot!(tree.rename(&pos_book_rename, "Kitab", contents));
        assert_yaml_snapshot!(tree.rename(&pos_author_rename, "Writer", contents));
        assert_yaml_snapshot!(tree.rename(&pos_non_renamble, "Doesn't matter", contents));
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
