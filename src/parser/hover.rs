use async_lsp::lsp_types::MarkedString;
use tree_sitter::Node;

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

    pub fn hover(&self, identifier: &str, content: impl AsRef<[u8]>) -> Vec<MarkedString> {
        let mut v = vec![];
        self.hover_impl(identifier, self.tree.root_node(), &mut v, content);
        v
    }

    fn hover_impl(
        &self,
        identifier: &str,
        n: Node,
        v: &mut Vec<MarkedString>,
        content: impl AsRef<[u8]>,
    ) {
        if identifier.is_empty() {
            return;
        }

        if !identifier.contains(".") {
            let comments: Vec<MarkedString> = self
                .filter_nodes_from(n, NodeKind::is_userdefined)
                .into_iter()
                .filter(|n| n.utf8_text(content.as_ref()).expect("utf-8 parse error") == identifier)
                .filter_map(|n| self.find_preceding_comments(n.id(), content.as_ref()))
                .map(MarkedString::String)
                .collect();

            v.extend(comments);
            return;
        }

        // Safety: identifier contains a .
        let (parent_identifier, remaining) = identifier.split_once(".").unwrap();
        let child_node = self
            .filter_nodes_from(n, NodeKind::is_userdefined)
            .into_iter()
            .find(|n| n.utf8_text(content.as_ref()).expect("utf8-parse error") == parent_identifier)
            .map(|n| n.parent().unwrap()); // Safety: All userdefined types would have a parent

        if let Some(inner) = child_node {
            self.hover_impl(remaining, inner, v, content);
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{MarkedString, Url};

    use crate::parser::ProtoParser;

    #[test]
    fn test_hover() {
        let uri: Url = "file://foo.bar/p.proto".parse().unwrap();
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
    };
}

// Comic is a type of book but who cares
message Comic {
    // Author of a comic is different from others
    message Author {
        string name = 1;
        string country = 2;
    };
}
"#;
        let parsed = ProtoParser::new().parse(uri.clone(), contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let res = tree.hover("Book", contents);

        assert_eq!(res.len(), 1);
        assert_eq!(res[0], MarkedString::String("A Book is book".to_owned()));

        let res = tree.hover("", contents);
        assert_eq!(res.len(), 0);

        let res = tree.hover("Book.Author", contents);
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

        let res = tree.hover("Comic.Author", contents);
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0],
            MarkedString::String("Author of a comic is different from others".to_owned())
        );

        let res = tree.hover("Author", contents);
        assert_eq!(res.len(), 2);
    }
}
