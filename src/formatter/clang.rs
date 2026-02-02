#![allow(clippy::needless_late_init)]
use std::{
    borrow::Cow,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use async_lsp::lsp_types::{Position, Range, TextEdit};
use hard_xml::XmlRead;
use serde::Serialize;
use tempfile::{TempDir, tempdir};

use super::ProtoFormatter;

pub struct ClangFormatter {
    pub path: String,
    working_dir: Option<String>,
    temp_dir: TempDir,
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
        let up_to_offset = &content[..offset];
        let line = up_to_offset.matches('\n').count();
        let last_newline = up_to_offset.rfind('\n').map_or(0, |pos| pos + 1);

        // LSP uses UTF-16 code units for character positions
        // Count UTF-16 code units from last newline to offset
        let text_after_newline = &up_to_offset[last_newline..];
        let character = text_after_newline.encode_utf16().count();

        Some(Position {
            line: line as u32,
            character: character as u32,
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

impl ClangFormatter {
    pub fn new(cmd: &str, wdir: Option<&str>) -> Self {
        Self {
            temp_dir: tempdir().expect("faile to creat temp dir"),
            path: cmd.to_owned(),
            working_dir: wdir.map(ToOwned::to_owned),
        }
    }

    fn get_temp_file_path(&self, content: &str) -> Option<PathBuf> {
        let p = self.temp_dir.path().join("format-temp.proto");
        let mut file = File::create(p.clone()).ok()?;
        file.write_all(content.as_ref()).ok()?;
        Some(p)
    }

    fn get_command(&self, f: &str, u: &Path) -> Option<Command> {
        let mut c = Command::new(self.path.as_str());
        if let Some(wd) = &self.working_dir {
            c.current_dir(wd.as_str());
        }
        c.stdin(File::open(u).ok()?);
        c.args([
            "--output-replacements-xml",
            format!("--assume-filename={f}").as_str(),
        ]);
        Some(c)
    }

    fn output_to_textedit(&self, output: &str, content: &str) -> Option<Vec<TextEdit>> {
        let r = Replacements::from_str(output).ok()?;
        let edits = r
            .replacements
            .into_iter()
            .filter_map(|r| r.as_text_edit(content.as_ref()))
            .collect();

        Some(edits)
    }
}

impl ProtoFormatter for ClangFormatter {
    fn format_document(&self, filename: &str, content: &str) -> Option<Vec<TextEdit>> {
        let p = self.get_temp_file_path(content)?;
        let mut cmd = self.get_command(filename, p.as_ref())?;
        let output = cmd.output().ok()?;
        if !output.status.success() {
            tracing::error!(
                status = output.status.code(),
                "failed to execute clang-format"
            );
            return None;
        }
        self.output_to_textedit(&String::from_utf8_lossy(&output.stdout), content)
    }

    fn format_document_range(
        &self,
        r: &Range,
        filename: &str,
        content: &str,
    ) -> Option<Vec<TextEdit>> {
        let p = self.get_temp_file_path(content)?;
        let start = r.start.line + 1;
        let end = r.end.line + 1;
        let output = self
            .get_command(filename, p.as_ref())?
            .args(["--lines", format!("{start}:{end}").as_str()])
            .output()
            .ok()?;

        if !output.status.success() {
            tracing::error!(
                status = output.status.code(),
                "failed to execute clang-format"
            );
            return None;
        }
        self.output_to_textedit(&String::from_utf8_lossy(&output.stdout), content)
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
            })
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
            })
        }
    }

    #[test]
    fn test_textedit_from_clang_output_cyrillic() {
        // Test that the complete flow works with Cyrillic characters
        // This simulates what clang-format would output for the Cyrillic comment
        let content = include_str!("input/test_cyrillic.proto");
        let xml_output = r#"<?xml version='1.0'?>
<replacements xml:space='preserve' incomplete_format='false'>
<replacement offset='134' length='1'>
  // </replacement>
</replacements>"#;

        let r = Replacements::from_str(xml_output).unwrap();
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
