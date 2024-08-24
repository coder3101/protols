use async_lsp::lsp_types::{Position, Range, TextEdit};

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

    pub fn rename_fields(
        &self,
        field_name: &str,
        new_identifier: &str,
        content: impl AsRef<[u8]>,
    ) -> Vec<TextEdit> {
        let renaming_field = field_name.split('.').last().unwrap_or(field_name);
        let new_field_name = field_name.replace(renaming_field, new_identifier);

        self.filter_nodes(NodeKind::is_field_name)
            .into_iter()
            .filter(|n| {
                n.utf8_text(content.as_ref())
                    .expect("utf-8 parse error")
                    .starts_with(field_name)
            })
            .map(|n| {
                let old_text = n.utf8_text(content.as_ref()).expect("utf-8 parse error");
                TextEdit {
                    new_text: old_text.replace(field_name, &new_field_name),
                    range: Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    },
                }
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
    fn test_rename_fields() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename.proto");

        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();

        assert_yaml_snapshot!(tree.rename_fields("Book", "Kitab", contents));
        assert_yaml_snapshot!(tree.rename_fields("Book.Author", "Writer", contents));
        assert_yaml_snapshot!(tree.rename_fields("xyz.abc", "Doesn't matter", contents));
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
