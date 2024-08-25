use std::{error::Error, process::Command};

use async_lsp::lsp_types::{Position, Range, TextEdit, Url};
use tracing::info;

use super::ProtoFormatter;

pub struct ClangFormatter {
    path: String,
    working_dir: String,
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
}

impl ProtoFormatter for ClangFormatter {
    fn format_document(&self, u: &Url) -> Option<Vec<TextEdit>> {
        let mut c = Command::new(self.path.as_str());
        c.current_dir(self.working_dir.as_str());
        let output = c.arg(u.path()).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let output = String::from_utf8_lossy(&output.stdout);
        Some(vec![TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: u32::MAX,
                    character: u32::MAX,
                },
            },
            new_text: output.to_string(),
        }])
    }

    fn format_document_range(
        &self,
        _u: &async_lsp::lsp_types::Url,
        _r: &async_lsp::lsp_types::Range,
    ) -> Option<Vec<async_lsp::lsp_types::TextEdit>> {
        todo!()
    }
}
