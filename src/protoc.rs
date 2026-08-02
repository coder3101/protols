use std::process::Command;

use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use tree_sitter::Point;

use crate::utils::to_lsp_position;

pub fn collect_diagnostics(
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
            if output.status.success() {
                Vec::new()
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                parse_protoc_output(&error)
            }
        }
        Err(e) => {
            tracing::error!(error=%e, "failed to run protoc");
            Vec::new()
        }
    }
}

// Visible for testing
fn parse_protoc_output(output: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        // Parse protoc error format: file:line:column: message
        if let Some((file_info, message)) = line.split_once(": ") {
            let parts: Vec<&str> = file_info.split(':').collect();
            if parts.len() >= 3
                && let (Ok(line), Ok(col)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>())
            {
                let point = Point {
                    row: (line - 1) as usize,
                    column: (col - 1) as usize,
                };
                let diagnostic = Diagnostic {
                    range: Range {
                        start: to_lsp_position(point),
                        end: to_lsp_position(Point {
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

    diagnostics
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_protoc_output_single_error() {
        let output = "foo.proto:5:3: Expected field name.\n";
        let diags = parse_protoc_output(output);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "Expected field name.");
        assert_eq!(diags[0].source, Some("protoc".to_string()));
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[0].range.start.line, 4);
        assert_eq!(diags[0].range.start.character, 2);
        assert_eq!(diags[0].range.end.line, 4);
        assert_eq!(diags[0].range.end.character, 3);
    }

    #[test]
    fn test_parse_protoc_output_multiple_errors() {
        let output = "a.proto:1:1: Syntax error.\nb.proto:2:3: Unknown type.\n";
        let diags = parse_protoc_output(output);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].message, "Syntax error.");
        assert_eq!(diags[1].message, "Unknown type.");
    }

    #[test]
    fn test_parse_protoc_output_empty() {
        assert!(parse_protoc_output("").is_empty());
    }

    #[test]
    fn test_parse_protoc_output_malformed_line() {
        let output = "not a valid protoc error line\n";
        let diags = parse_protoc_output(output);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_parse_protoc_output_partial_format() {
        // Missing column number
        let output = "foo.proto:5: Expected field name.\n";
        let diags = parse_protoc_output(output);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_parse_protoc_output_non_numeric_line_col() {
        let output = "foo.proto:abc:def: some message\n";
        let diags = parse_protoc_output(output);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_parse_protoc_output_message_with_colons() {
        let output = "foo.proto:3:1: 'Foo' is not defined. It could be a typo for 'Bar'.\n";
        let diags = parse_protoc_output(output);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            "'Foo' is not defined. It could be a typo for 'Bar'."
        );
    }
}
