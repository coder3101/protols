use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, PublishDiagnosticsParams, Range};

use crate::{nodekind::NodeKind, utils::ts_to_lsp_position};

use super::ParsedTree;

impl ParsedTree {
    pub fn collect_parse_errors(&self) -> PublishDiagnosticsParams {
        let diagnostics = self
            .filter_nodes(NodeKind::is_error)
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
            uri: self.uri.clone(),
            diagnostics,
            version: None,
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use insta::assert_yaml_snapshot;

    use crate::parser::ProtoParser;

    #[test]
    fn test_collect_parse_error() {
        let url: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_collect_parse_error1.proto");

        let parsed = ProtoParser::new().parse(url.clone(), contents);
        assert!(parsed.is_some());
        assert_yaml_snapshot!(parsed.unwrap().collect_parse_errors());

        let contents = include_str!("input/test_collect_parse_error2.proto");

        let parsed = ProtoParser::new().parse(url.clone(), contents);
        assert!(parsed.is_some());
        assert_yaml_snapshot!(parsed.unwrap().collect_parse_errors());
    }
}
