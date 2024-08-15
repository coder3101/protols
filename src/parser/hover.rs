use async_lsp::lsp_types::{MarkedString, Position};
use tracing::info;


use crate::parser::nodekind::NodeKind;

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

    pub fn hover(&self, pos: &Position, content: impl AsRef<[u8]>) -> Vec<MarkedString> {
        let text = self.get_actionable_node_text_at_position(pos, content.as_ref());
        info!("Looking for hover response on: {:?}", text);

        match text {
            Some(text) => self
                .filter_node(NodeKind::is_actionable)
                .into_iter()
                .filter(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error") == text)
                .filter_map(|n| self.find_preceding_comments(n.id(), content.as_ref()))
                .map(MarkedString::String)
                .collect(),
            None => vec![],
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{MarkedString, Position};

    use crate::parser::ProtoParser;

    #[test]
    fn test_hover() {
        let posbook = Position {
            line: 5,
            character: 9,
        };
        let posinvalid = Position {
            line: 0,
            character: 1,
        };
        let posauthor = Position {
            line: 11,
            character: 14,
        };
        let posts = Position {
            line: 14,
            character: 14,
        };
        let contents = r#"syntax = "proto3";

package com.book;

// A Book is book
message Book {

    // This is represents author
    // A author is a someone who writes books
    //
    // Author has a name and a country where they were born
    message Author {
        string name = 1;
        string country = 2;
        google.protobuf.Type ts = 3;
    };
}
"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let res = tree.hover(&posbook, contents);

        assert_eq!(res.len(), 1);
        assert_eq!(res[0], MarkedString::String("A Book is book".to_owned()));

        let res = tree.hover(&posinvalid, contents);
        assert_eq!(res.len(), 0);

        let res = tree.hover(&posauthor, contents);
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0],
            MarkedString::String(
                r#"This is represents author
A author is a someone who writes books

Author has a name and a country where they were born"#
                    .to_owned()
            )
        );

        let res = tree.hover(&posts, contents);
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0],
            MarkedString::String("A protocol buffer message type".to_owned())
        )
    }
}
