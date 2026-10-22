//! Syntax-level diagnostics that still require direct access to the raw
//! Tree-sitter tree.
//!
//! Parse errors (`ERROR` nodes) are not part of the semantic metamodel — the
//! extractor only records well-formed entities — so collecting them is the one
//! place we traverse the raw syntax tree directly.

use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};
use tree_sitter::TreeCursor;

use crate::utils::to_lsp_range;

use super::parser::ProtoDocument;

impl ProtoDocument {
    /// Collects parse diagnostics by walking the raw syntax tree for `ERROR`
    /// nodes.
    pub fn collect_parse_diagnostics(&self) -> Vec<Diagnostic> {
        let mut errors = Vec::new();
        let mut cursor = self.tree.walk();

        loop {
            let node = cursor.node();
            let is_missing = node.is_missing();

            if node.is_error() || is_missing {
                errors.push(Diagnostic {
                    range: to_lsp_range(node),
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("protols".into()),
                    message: if is_missing {
                        format!("Missing syntax element: {}", node.kind())
                    } else {
                        "Syntax error".into()
                    },
                    ..Default::default()
                });

                if advance_to_next(&mut cursor) {
                    continue;
                }

                break;
            }

            if cursor.goto_first_child() {
                continue;
            }

            if advance_to_next(&mut cursor) {
                continue;
            }

            break;
        }

        errors
    }
}

/// Advances the cursor to the next node in the traversal, bypassing the current
/// node's children.
///
/// Returns `true` if advanced to a sibling or a parent's sibling, or `false` if
/// the traversal is complete.
#[inline]
fn advance_to_next(cursor: &mut TreeCursor) -> bool {
    while !cursor.goto_next_sibling() {
        if !cursor.goto_parent() {
            return false;
        }
    }
    true
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
