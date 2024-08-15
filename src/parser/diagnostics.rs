use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, PublishDiagnosticsParams, Range, Url};

use crate::utils::ts_to_lsp_position;

use super::{nodekind::NodeKind, ParsedTree};

impl ParsedTree {
    pub fn collect_parse_errors(&self, uri: &Url) -> PublishDiagnosticsParams {
        let diagnostics = self
            .filter_node(NodeKind::is_error)
            .into_iter()
            .map(|n| Diagnostic {
                range: Range {
                    start: ts_to_lsp_position(&n.start_position()),
                    end: ts_to_lsp_position(&n.end_position()),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("protols".to_string()),
                message: "Syntax error".to_string(),
                ..Default::default()
            })
            .collect();
        PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: None,
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{DiagnosticSeverity, Position, Range, Url};

    use crate::parser::ProtoParser;

    #[test]
    fn test_collect_parse_error() {
        let url = "file://foo/bar.proto";
        let contents = r#"syntax = "proto3";

package test;

message Foo {
	reserved 1;
	reserved "baz";
	int bar = 2;
}"#;

        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let diagnostics = tree.collect_parse_errors(&url.parse().unwrap());
        assert_eq!(diagnostics.uri, Url::parse(url).unwrap());
        assert_eq!(diagnostics.diagnostics.len(), 0);

        let url = "file://foo/bar.proto";
        let contents = r#"syntax = "proto3";

package com.book;

message Book {
    message Author {
        string name;
        string country = 2;
    };
}"#;
        let parsed = ProtoParser::new().parse(contents);
        assert!(parsed.is_some());
        let tree = parsed.unwrap();
        let diagnostics = tree.collect_parse_errors(&url.parse().unwrap());

        assert_eq!(diagnostics.uri, Url::parse(url).unwrap());
        assert_eq!(diagnostics.diagnostics.len(), 1);

        let error = &diagnostics.diagnostics[0];
        assert_eq!(error.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(error.source, Some("protols".to_owned()));
        assert_eq!(error.message, "Syntax error");
        assert_eq!(
            error.range,
            Range {
                start: Position {
                    line: 6,
                    character: 8
                },
                end: Position {
                    line: 6,
                    character: 19
                }
            }
        );
    }
}
