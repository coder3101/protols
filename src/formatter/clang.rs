use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;
use std::{borrow::Cow, process::Stdio};

use async_lsp::lsp_types::{Position, Range, TextEdit};
use hard_xml::XmlRead;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub struct ClangFormatter {
    pub path: OsString,
    working_dir: Option<PathBuf>,
}

#[derive(XmlRead, Serialize, PartialEq, Debug)]
#[xml(tag = "replacements")]
struct Replacements<'a> {
    #[xml(child = "replacement")]
    replacements: Vec<Replacement<'a>>,
}

#[derive(XmlRead, Serialize, PartialEq, Debug)]
#[xml(tag = "replacement")]
struct Replacement<'a> {
    #[xml(attr = "offset")]
    offset: usize,
    #[xml(attr = "length")]
    length: usize,
    #[xml(text)]
    text: Cow<'a, str>,
}

impl Replacement<'_> {
    fn offset_to_position(offset: usize, content: &str) -> Option<Position> {
        if offset > content.len() {
            return None;
        }

        // Use floor_char_boundary to ensure we don't slice in the middle of a
        // multi-byte UTF-8 character (e.g., Cyrillic), which would cause a panic.
        // This handles slight offset shifts caused by different OS line endings.
        let safe_offset = content.floor_char_boundary(offset);

        let up_to_offset = &content[..safe_offset];
        let line = up_to_offset.matches('\n').count();
        let last_newline = up_to_offset.rfind('\n').map_or(0, |pos| pos + 1);

        // LSP uses UTF-16 code units for character positions
        // Count UTF-16 code units from last newline to offset
        let text_after_newline = &up_to_offset[last_newline..];
        let character = text_after_newline.encode_utf16().count();

        Some(Position {
            line: u32::try_from(line).ok()?,
            character: u32::try_from(character).ok()?,
        })
    }

    fn as_text_edit(&self, content: &str) -> Option<TextEdit> {
        Some(TextEdit {
            range: Range {
                start: Self::offset_to_position(self.offset, content)?,
                end: Self::offset_to_position(self.offset + self.length, content)?,
            },
            new_text: self.text.to_string(),
        })
    }
}

const CLANG_FORMAT_TIMEOUT: Duration = Duration::from_secs(5);

impl ClangFormatter {
    pub fn new(cmd: OsString, working_dir: Option<PathBuf>) -> Self {
        Self {
            path: cmd,
            working_dir,
        }
    }

    /// # Cancellation safety
    ///
    /// This method is cancel safe.
    pub async fn format_document(&self, filename: &str, content: &str) -> Option<Vec<TextEdit>> {
        let output = self.run_clang_format(filename, content, &[]).await?;
        Self::output_to_textedit(&output, content)
    }

    /// # Cancellation safety
    ///
    /// This method is cancel safe.
    pub async fn format_document_range(
        &self,
        r: &Range,
        filename: &str,
        content: &str,
    ) -> Option<Vec<TextEdit>> {
        let start = r.start.line + 1;
        let end = r.end.line + 1;
        let line_arg = format!("{start}:{end}");
        let extra_args = ["--lines", &line_arg];

        let output = self
            .run_clang_format(filename, content, &extra_args)
            .await?;
        Self::output_to_textedit(&output, content)
    }

    /// # Cancellation safety
    ///
    /// This method is cancel safe.
    async fn run_clang_format(
        &self,
        filename: &str,
        content: &str,
        extra_args: &[&str],
    ) -> Option<String> {
        let mut c = Command::new(&self.path);

        if let Some(wd) = &self.working_dir {
            c.current_dir(wd);
        }

        c.stdin(Stdio::piped());
        c.stdout(Stdio::piped());
        c.stderr(Stdio::piped());

        c.args([
            "--output-replacements-xml",
            &format!("--assume-filename={filename}"),
        ]);

        c.args(extra_args);

        let process = async {
            let mut child = c.kill_on_drop(true).spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(content.as_bytes()).await?;
            }

            child.wait_with_output().await
        };

        let timeout_result = tokio::time::timeout(CLANG_FORMAT_TIMEOUT, process).await;

        let output = match timeout_result {
            Ok(Ok(out)) => out,
            Ok(Err(error)) => {
                tracing::error!(%error, "failed to run clang-format");
                return None;
            }
            Err(_elapsed) => {
                tracing::error!(
                    filename,
                    timeout_ms = CLANG_FORMAT_TIMEOUT.as_millis(),
                    "clang-format execution timed out and was killed"
                );
                return None;
            }
        };

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            tracing::error!(
                status = output.status.code(),
                error = %err_msg,
                "failed to execute clang-format"
            );
            return None;
        }

        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn output_to_textedit(output: &str, content: &str) -> Option<Vec<TextEdit>> {
        let r = Replacements::from_str(output).ok()?;
        let edits = r
            .replacements
            .into_iter()
            .filter_map(|r| r.as_text_edit(content))
            .collect();

        Some(edits)
    }
}

#[cfg(test)]
mod test {
    use hard_xml::XmlRead;
    use insta::{assert_yaml_snapshot, with_settings};

    use super::{Replacement, Replacements};

    #[test]
    fn test_reading_xml() {
        let c = include_str!("input/replacement.xml");
        let r = Replacements::from_str(c).unwrap();
        assert_yaml_snapshot!(r);
    }

    #[test]
    fn test_reading_empty_xml() {
        let c = include_str!("input/empty.xml");
        let r = Replacements::from_str(c).unwrap();
        assert_yaml_snapshot!(r);
    }

    #[test]
    fn test_offset_to_position() {
        let c = include_str!("input/test.proto");
        let pos = vec![0, 4, 22, 999];
        for i in pos {
            with_settings!({description => c, info => &i}, {
                assert_yaml_snapshot!(Replacement::offset_to_position(i, c));
            });
        }
    }

    #[test]
    fn test_offset_to_position_cyrillic() {
        // Test with Cyrillic characters (multi-byte UTF-8)
        let c = include_str!("input/test_cyrillic.proto");
        // Byte offset 134 corresponds to UTF-16 code unit 77 from the start of line 1
        // (the comment line contains multi-byte UTF-8 characters)
        let pos = vec![0, 15, 134];
        for i in pos {
            with_settings!({description => c, info => &i}, {
                assert_yaml_snapshot!(Replacement::offset_to_position(i, c));
            });
        }
    }

    #[test]
    fn test_textedit_from_clang_output_cyrillic() {
        // Test that the complete flow works with Cyrillic characters
        // This simulates what clang-format would output for the Cyrillic comment
        let content = include_str!("input/test_cyrillic.proto");

        // We use a dynamic offset instead of a hardcoded byte index (like 134)
        // because Windows uses CRLF (\r\n) while Linux uses LF (\n).
        // Git's autocrlf can shift byte positions on Windows, potentially
        // landing a fixed offset in the middle of a multi-byte UTF-8 character
        // (like Cyrillic). Finding the target string in memory ensures we hit
        // the correct character boundary regardless of the OS line endings.
        let target = " removed_not_true";
        let offset = content
            .find(target)
            .expect("Could not find target in content");
        let xml_output = format!(
            r"<?xml version='1.0'?>
<replacements xml:space='preserve' incomplete_format='false'>
<replacement offset='{offset}' length='1'>
  // </replacement>
</replacements>"
        );

        let r = Replacements::from_str(&xml_output).unwrap();
        assert_eq!(r.replacements.len(), 1);

        let replacement = &r.replacements[0];
        assert_eq!(replacement.offset, 134);
        assert_eq!(replacement.length, 1);

        let text_edit = replacement.as_text_edit(content).unwrap();
        // The edit should be at line 1, character 77 (not 119)
        assert_eq!(text_edit.range.start.line, 1);
        assert_eq!(text_edit.range.start.character, 77);
        assert_eq!(text_edit.range.end.line, 1);
        assert_eq!(text_edit.range.end.character, 78);
    }
}
