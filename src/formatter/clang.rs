#![allow(clippy::needless_late_init)]
use std::{
    borrow::Cow,
    error::Error,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use async_lsp::lsp_types::{Position, Range, TextEdit};
use hard_xml::XmlRead;
use serde::Serialize;
use tempfile::{tempdir, TempDir};

use super::ProtoFormatter;

pub struct ClangFormatter {
    path: String,
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

impl<'a> Replacement<'a> {
    fn offset_to_position(offset: usize, content: &str) -> Option<Position> {
        if offset > content.len() {
            return None;
        }
        let up_to_offset = &content[..offset];
        let line = up_to_offset.matches('\n').count();
        let last_newline = up_to_offset.rfind('\n').map_or(0, |pos| pos + 1);
        let character = offset - last_newline;

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
    pub fn new(path: &str, workdir: Option<&str>) -> Result<Self, Box<dyn Error>> {
        let mut c = Command::new(path);
        c.arg("--version").status()?;

        Ok(Self {
            temp_dir: tempdir()?,
            path: path.to_owned(),
            working_dir: workdir.map(ToOwned::to_owned),
        })
    }

    fn get_temp_file_path(&self, content: &str) -> Option<PathBuf> {
        let p = self.temp_dir.path().join("format-temp.proto");
        let mut file = File::create(p.clone()).ok()?;
        file.write_all(content.as_ref()).ok()?;
        Some(p)
    }

    fn get_command(&self, f: &str, u: &Path) -> Option<Command> {
        let mut c = Command::new(self.path.as_str());
        if let Some(wd) = self.working_dir.as_ref() {
            c.current_dir(wd.as_str());
        }
        c.stdin(File::open(u).ok()?);
        c.args(["--output-replacements-xml", format!("--assume-filename={f}").as_str()]);
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

    fn format_document_range(&self, r: &Range, filename: &str, content: &str) -> Option<Vec<TextEdit>> {
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
}
