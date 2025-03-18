use crate::utils::ts_to_lsp_position;
use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use std::process::Command;
use tree_sitter::Point;

pub struct ProtocDiagnostics {}

impl ProtocDiagnostics {
    pub fn new() -> Self {
        Self {}
    }

    pub fn collect_diagnostics(
        &self,
        protoc_path: &str,
        file_path: &str,
        include_paths: &[String],
    ) -> Vec<Diagnostic> {
        let mut cmd = Command::new(protoc_path);

        // Add include paths
        for path in include_paths {
            cmd.arg("-I").arg(path);
        }

        // Generate descriptor but discard its output
        cmd.arg("-o")
            .arg(if cfg!(windows) { "NUL" } else { "/dev/null" });

        // Add the file to check
        cmd.arg(file_path);

        // Run protoc and capture output
        match cmd.output() {
            Ok(output) => {
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr);
                    self.parse_protoc_output(&error)
                } else {
                    Vec::new()
                }
            }
            Err(e) => {
                tracing::error!(error=%e, "failed to run protoc");
                Vec::new()
            }
        }
    }

    fn parse_protoc_output(&self, output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for line in output.lines() {
            // Parse protoc error format: file:line:column: message
            if let Some((file_info, message)) = line.split_once(": ") {
                let parts: Vec<&str> = file_info.split(':').collect();
                if parts.len() >= 3 {
                    if let (Ok(line), Ok(col)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>())
                    {
                        let point = Point {
                            row: (line - 1) as usize,
                            column: (col - 1) as usize,
                        };
                        let diagnostic = Diagnostic {
                            range: Range {
                                start: ts_to_lsp_position(&point),
                                end: ts_to_lsp_position(&Point {
                                    row: point.row,
                                    column: point.column + 1,
                                }),
                            },
                            severity: Some(DiagnosticSeverity::ERROR),
                            source: Some("protoc".to_string()),
                            message: message.to_string(),
                            ..Default::default()
                        };
                        diagnostics.push(diagnostic);
                    }
                }
            }
        }

        diagnostics
    }
}
