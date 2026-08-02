use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use super::parser::ProtoDocument;

impl ProtoDocument {
    pub fn collect_import_diagnostics(&self, import: &[&str]) -> Vec<Diagnostic> {
        self.import_path_ranges(import)
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
