//! Syntax-level diagnostics that still require direct access to the raw
//! Tree-sitter tree.
//!
//! Parse errors (`ERROR` nodes) are not part of the semantic metamodel — the
//! extractor only records well-formed entities — so collecting them is the one
//! place we traverse the raw syntax tree directly.

use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use tree_sitter::Node;

use crate::{nodekind::NodeKind, utils::to_lsp_range};

use super::parser::ProtoDocument;

impl ProtoDocument {
    /// Collects parse diagnostics by walking the raw syntax tree for `ERROR`
    /// nodes.
    pub fn collect_parse_diagnostics(&self) -> Vec<Diagnostic> {
        let mut errors = Vec::new();
        collect_error_nodes(self.tree.root_node(), &mut errors);

        errors
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
}

fn collect_error_nodes<'a>(n: Node<'a>, out: &mut Vec<Node<'a>>) {
    if NodeKind::is_error(&n) {
        out.push(n);
    }
    let mut cursor = n.walk();
    for child in n.children(&mut cursor) {
        collect_error_nodes(child, out);
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use insta::assert_yaml_snapshot;

    use crate::document::parser::ProtoParser;
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
