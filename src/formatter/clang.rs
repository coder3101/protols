use std::{borrow::Cow, error::Error, fs::File, io::Write, path::PathBuf, process::Command};

use async_lsp::lsp_types::{Position, Range, TextEdit, Url};
use hard_xml::XmlRead;
use tempfile::{tempdir, TempDir};

use super::ProtoFormatter;

pub struct ClangFormatter {
    path: String,
    working_dir: String,
    temp_dir: TempDir,
}

#[derive(XmlRead, PartialEq, Debug)]
#[xml(tag = "replacements")]
struct Replacements<'a> {
    #[xml(child = "replacement")]
    replacements: Vec<Replacement<'a>>,
}

#[derive(XmlRead, PartialEq, Debug)]
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

        tracing::info!(line, character);

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
    pub fn new(path: &str, workdir: &str) -> Result<Self, Box<dyn Error>> {
        let mut c = Command::new(path);
        c.arg("--version").status()?;

        Ok(Self {
            temp_dir: tempdir()?,
            path: path.to_owned(),
            working_dir: workdir.to_owned(),
        })
    }

    fn get_temp_file_path(&self, content: &str) -> Option<PathBuf> {
        let p = self.temp_dir.path().join("");
        let mut file = File::create(p.clone()).ok()?;
        file.write_all(content.as_ref()).ok()?;
        return Some(p);
    }

    fn get_command(&self, u: &PathBuf) -> Command {
        let mut c = Command::new(self.path.as_str());
        c.current_dir(self.working_dir.as_str());
        c.args([u.as_path().to_str().unwrap(), "--output-replacements-xml"]);
        c
    }

    fn output_to_textedit(&self, output: &str, content: &str) -> Option<Vec<TextEdit>> {
        let r = Replacements::from_str(&output).ok()?;
        tracing::info!("{r:?}");
        let edits = r
            .replacements
            .into_iter()
            .filter_map(|r| r.as_text_edit(content.as_ref()))
            .collect();

        tracing::info!("{edits:?}");
        Some(edits)
    }
}

impl ProtoFormatter for ClangFormatter {
    fn format_document(&self, content: &str) -> Option<Vec<TextEdit>> {
        let p = self.get_temp_file_path(content)?;
        let output = self.get_command(&p).output().ok()?;
        if !output.status.success() {
            return None;
        }
        self.output_to_textedit(&String::from_utf8_lossy(&output.stdout), content)
    }

    fn format_document_range(&self, r: &Range, content: &str) -> Option<Vec<TextEdit>> {
        let p = self.get_temp_file_path(content)?;
        let start = r.start.line + 1;
        let end = r.end.line + 1;
        let output = self
            .get_command(&p)
            .args(["--lines", format!("{start}:{end}").as_str()])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }
        self.output_to_textedit(&String::from_utf8_lossy(&output.stdout), content)
    }
}
