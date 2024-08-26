use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use std::{error::Error, process::Command};

use async_lsp::lsp_types::{Position, Range, TextEdit, Url};

use super::ProtoFormatter;

pub struct ClangFormatter {
    path: String,
    working_dir: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Replacements {
    replacements: Vec<Replacement>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Replacement {
    offset: u32,
    length: u32,
    value: String,
}

impl ClangFormatter {
    pub fn new(path: &str, workdir: &str) -> Result<Self, Box<dyn Error>> {
        let mut c = Command::new(path);
        c.arg("--version").status()?;

        Ok(Self {
            path: path.to_owned(),
            working_dir: workdir.to_owned(),
        })
    }

    fn get_command(&self, u: &Url) -> Command {
        let mut c = Command::new(self.path.as_str());
        c.current_dir(self.working_dir.as_str());
        c.args([u.path(), "--output-replacements-xml"]);
        c
    }
}

impl ProtoFormatter for ClangFormatter {
    fn format_document(&self, u: &Url) -> Option<Vec<TextEdit>> {
        let output = self.get_command(u).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let output = String::from_utf8_lossy(&output.stdout);

        let out: Replacements = from_str(&output).ok()?;
        let edits = out
            .replacements
            .into_iter()
            .map(|r| TextEdit {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 1,
                        character: 3,
                    },
                },
                new_text: r.value,
            })
            .collect();

        tracing::info!("{edits:?}");
        Some(edits)
    }

    fn format_document_range(
        &self,
        u: &Url,
        r: &Range,
    ) -> Option<Vec<async_lsp::lsp_types::TextEdit>> {
        let start = r.start.line + 1;
        let end = r.end.line + 1;
        let output = self
            .get_command(u)
            .args(["--lines", format!("{start}:{end}").as_str()])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let output = String::from_utf8_lossy(&output.stdout);

        tracing::info!("{output}");
        if let Err(e) = from_str::<Replacements>(&output) {
            tracing::error!("{e}");
            return None;
        }
        let out: Replacements = from_str(&output).ok()?;
        let edits = out
            .replacements
            .into_iter()
            .map(|r| TextEdit {
                range: Range {
                    start: Position {
                        line: 9,
                        character: 1,
                    },
                    end: Position {
                        line: 12,
                        character: 6,
                    },
                },
                new_text: r.value,
            })
            .collect();

        tracing::info!("{edits:?}");
        Some(edits)
    }
}
