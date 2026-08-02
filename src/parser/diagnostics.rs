use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::{nodekind::NodeKind, utils::to_lsp_range};

use super::ParsedTree;

impl ParsedTree {
    pub fn collect_parse_diagnostics(&self) -> Vec<Diagnostic> {
        self.find_all_nodes(NodeKind::is_error)
            .into_iter()
            .map(|n| Diagnostic {
                range: to_lsp_range(n),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("protols".to_string()),
                message: "Syntax error".to_string(),
                ..Default::default()
            })
            .collect()
    }

    pub fn collect_import_diagnostics(&self, content: &[u8], import: &[&str]) -> Vec<Diagnostic> {
        self.get_import_path_range(content, import)
            .into_iter()
            .map(|r| Diagnostic {
                range: r,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some(String::from("protols")),
                message: "failed to find proto file".to_string(),
                ..Default::default()
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use insta::assert_yaml_snapshot;

    use crate::parser::ProtoParser;
    use crate::utils::compile_test_query;

    #[test]
    fn test_collect_parse_error() {
        let url: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_collect_parse_error1.proto");
        let query = &compile_test_query();

        let parsed = ProtoParser::new().parse(url.clone(), contents, query);
        assert!(parsed.is_some());
        assert_yaml_snapshot!(parsed.unwrap().collect_parse_diagnostics());

        let contents = include_str!("input/test_collect_parse_error2.proto");

        let parsed = ProtoParser::new().parse(url, contents, query);
        assert!(parsed.is_some());
        assert_yaml_snapshot!(parsed.unwrap().collect_parse_diagnostics());
    }
}
