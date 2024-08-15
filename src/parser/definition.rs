use async_lsp::lsp_types::{Location, Position, Range, Url};
use tracing::info;

use crate::{parser::nodekind::NodeKind, utils::ts_to_lsp_position};

use super::ParsedTree;

impl ParsedTree {
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
                .filter_node(NodeKind::is_userdefined)
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
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Position, Range, Url};

    use crate::parser::ProtoParser;

    #[test]
    fn test_goto_definition() {
        let url = "file://foo/bar.proto";
        let posinvalid = Position {
            line: 0,
            character: 1,
        };
        let posauthor = Position {
            line: 10,
            character: 5,
        };
        let contents = r#"syntax = "proto3";

package com.book;

message Book {
    message Author {
        string name = 1;
        string country = 2;
    };

    Author author = 1;
    string isbn = 2;
}
"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let res = tree.definition(&posauthor, &url.parse().unwrap(), contents);

        assert_eq!(res.len(), 1);
        assert_eq!(res[0].uri, Url::parse(url).unwrap());
        assert_eq!(
            res[0].range,
            Range {
                start: Position {
                    line: 5,
                    character: 12
                },
                end: Position {
                    line: 5,
                    character: 18
                },
            }
        );

        let res = tree.definition(&posinvalid, &url.parse().unwrap(), contents);
        assert_eq!(res.len(), 0);
    }
}
