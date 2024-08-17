use async_lsp::lsp_types::{Location, Position, Range};
use tracing::info;

use crate::{parser::nodekind::NodeKind, utils::ts_to_lsp_position};

use super::ParsedTree;

impl ParsedTree {
    pub fn definition(&self, pos: &Position, content: impl AsRef<[u8]>) -> Vec<Location> {
        let text = self.get_node_text_at_position(pos, content.as_ref());
        info!("Looking for definition of: {:?}", text);

        match text {
            Some(text) => self
                .filter_nodes(NodeKind::is_userdefined)
                .into_iter()
                .filter(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error") == text)
                .map(|n| Location {
                    uri: self.uri.clone(),
                    range: Range {
                        start: ts_to_lsp_position(&n.start_position()),
                        end: ts_to_lsp_position(&n.end_position()),
                    },
                })
                .collect(),
            None => vec![],
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Position, Url};
    use insta::assert_yaml_snapshot;

    use crate::parser::ProtoParser;

    #[test]
    fn test_goto_definition() {
        let url: Url = "file://foo/bar.proto".parse().unwrap();
        let posinvalid = Position {
            line: 0,
            character: 1,
        };
        let posauthor = Position {
            line: 10,
            character: 5,
        };
        let contents = include_str!("input/test_goto_definition.proto");
        let parsed = ProtoParser::new().parse(url, contents);

        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        assert_yaml_snapshot!(tree.definition(&posauthor, contents));
        assert_yaml_snapshot!(tree.definition(&posinvalid, contents));
    }
}
