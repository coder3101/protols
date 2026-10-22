use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

use async_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use tokio::process::Command;
use tree_sitter::Point;

use crate::utils::to_lsp_position;

const PROTOC_TIMEOUT: Duration = Duration::from_secs(5);

/// # Cancellation safety
///
/// This method is cancel safe.
pub async fn collect_diagnostics<I, P>(
    protoc_path: &Path,
    file_path: &Path,
    include_paths: I,
) -> Vec<Diagnostic>
where
    I: IntoIterator<Item = P>,
    P: AsRef<OsStr>,
{
    let mut cmd = Command::new(protoc_path);

    // Add include paths
    for path in include_paths {
        cmd.arg("--proto_path").arg(path);
    }

    // Generate descriptor but discard its output
    cmd.arg("--descriptor_set_out")
        .arg(if cfg!(windows) { "NUL" } else { "/dev/null" })
        // Add the file to check
        .arg(file_path)
        .kill_on_drop(true);

    // Run protoc and capture output
    let Ok(cmd_result) = tokio::time::timeout(PROTOC_TIMEOUT, cmd.output()).await else {
        tracing::warn!(
            timeout_ms = PROTOC_TIMEOUT.as_millis(),
            "protoc execution timed out and was killed",
        );
        return Vec::new();
    };

    let Ok(output) =
        cmd_result.inspect_err(|error| tracing::error!(%error, "failed to run protoc"))
    else {
        return Vec::new();
    };

    if output.status.success() {
        return Vec::new();
    }

    parse_protoc_output(&String::from_utf8_lossy(&output.stderr))
}

// Visible for testing
fn parse_protoc_output(output: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        // Parse protoc error format: file:line:column: message
        let Some((file_info, message)) = line.split_once(": ") else {
            continue;
        };

        let Some((rest, col_str)) = file_info.rsplit_once(':') else {
            continue;
        };

        let Some((_file_name, line_str)) = rest.rsplit_once(':') else {
            continue;
        };

        if let (Ok(line), Ok(col)) = (line_str.parse::<u32>(), col_str.parse::<u32>()) {
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
                source: Some("protoc".to_owned()),
                message: message.to_owned(),
                ..Default::default()
            };
            diagnostics.push(diagnostic);
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
        assert_eq!(diags[0].source, Some("protoc".to_owned()));
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
        assert_eq!(parse_protoc_output(""), [] as [Diagnostic; 0]);
    }

    #[test]
    fn test_parse_protoc_output_malformed_line() {
        let output = "not a valid protoc error line\n";
        let diags = parse_protoc_output(output);
        assert_eq!(diags, [] as [Diagnostic; 0]);
    }

    #[test]
    fn test_parse_protoc_output_partial_format() {
        // Missing column number
        let output = "foo.proto:5: Expected field name.\n";
        let diags = parse_protoc_output(output);
        assert_eq!(diags, [] as [Diagnostic; 0]);
    }

    #[test]
    fn test_parse_protoc_output_non_numeric_line_col() {
        let output = "foo.proto:abc:def: some message\n";
        let diags = parse_protoc_output(output);
        assert_eq!(diags, [] as [Diagnostic; 0]);
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

    #[test]
    fn test_parse_protoc_output_windows_absolute_path() {
        let row = 10u32;
        let col = 5u32;
        let output =
            format!("C:\\Users\\Admin\\project\\foo.proto:{row}:{col}: Field type not found.\n");
        let diags = parse_protoc_output(&output);

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "Field type not found.");
        assert_eq!(diags[0].source, Some("protoc".to_owned()));
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));

        assert_eq!(diags[0].range.start.line, row - 1);
        assert_eq!(diags[0].range.start.character, col - 1);
    }
}
