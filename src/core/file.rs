use crate::core::utils::*;
use anyhow::{anyhow, Result};
use async_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};
use tree_sitter::Node;

use crate::utils;

use super::{ast::ProtoAST, query::*, symbol::ProtoSymbol};

#[derive(Clone)]
pub struct ProtoFile {
    uri: Url,
    text: String,
    ast: ProtoAST,
}

impl ProtoFile {
    fn get_node_text(&self, n: &Node<'_>) -> &str {
        n.utf8_text(self.text.as_bytes())
            .expect("utf8-text parse error")
    }
}

impl ProtoFile {
    pub fn new(uri: Url, text: &str) -> Result<Self> {
        Ok(Self {
            uri,
            text: text.to_string(),
            ast: ProtoAST::new(text)?,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn edit(&mut self, edits: Vec<TextDocumentContentChangeEvent>) -> Result<()> {
        for edit in edits {
            let r = edit.range.ok_or(anyhow!("empty range for edit"))?;
            let mut lines = self.text.split_inclusive('\n').peekable();

            let start_bytes: usize = lines
                .by_ref()
                .take(r.start.line as usize)
                .map(str::len)
                .sum();

            let start_offset = lines
                .peek()
                .map(|l| char_to_byte(&l, r.start.character))
                .unwrap_or(0);

            let start_bytes = start_bytes + start_offset;

            let end_bytes = start_bytes
                + lines
                    .by_ref()
                    .take((r.end.line - r.start.line) as usize)
                    .map(str::len)
                    .sum::<usize>();

            let end_offset = lines
                .peek()
                .map(|l| char_to_byte(&l, r.end.character))
                .unwrap_or(0);

            let end_bytes = end_bytes + end_offset - start_offset;

            self.text.replace_range(start_bytes..end_bytes, &edit.text);
        }
        self.ast.update(&self.text)?;
        Ok(())
    }

    pub fn reset(&mut self, new_text: &str) -> Result<()> {
        self.text = new_text.to_string();
        self.ast.update(new_text)
    }

    pub fn update_uri(&mut self, new_uri: Url) {
        self.uri = new_uri;
    }

    pub fn package(&self) -> Option<&str> {
        self.ast
            .query(&QUERY_PACKAGE_NAME, self.text.as_ref())
            .get(0)
            .map(|n| self.get_node_text(n))
    }

    pub fn imports(&self) -> Vec<&str> {
        self.ast
            .query(&QUERY_IMPORTS, self.text.as_ref())
            .into_iter()
            .map(|n| self.get_node_text(&n).trim_matches('"'))
            .collect()
    }

    pub fn symbols(&self) -> Vec<ProtoSymbol> {
        self.ast.symbols(self.text.as_ref())
    }

    pub fn symbols_relative_to(&self, base: &str) -> Vec<ProtoSymbol> {
        self.symbols()
            .into_iter()
            .map(|s| ProtoSymbol {
                text: relativise(base, &s.text).to_string(),
                ..s
            })
            .collect()
    }
}

#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use super::ProtoFile;

    #[test]
    fn test_package_name() {
        let c = include_str!("test/package_name.proto");
        let p = ProtoFile::new("file://test/package_name.proto".parse().unwrap(), c);

        assert!(p.is_ok());
        assert_eq!(p.unwrap().package(), Some("a.c.b"))
    }

    #[test]
    fn test_imports() {
        let c = include_str!("test/import.proto");
        let p = ProtoFile::new("file://test/import.proto".parse().unwrap(), c);

        assert!(p.is_ok());
        assert_eq!(
            p.unwrap().imports(),
            vec![
                "a.proto",
                "nested/supath.proto",
                "../relative/path.proto",
                "secondline/pt.proto"
            ]
        )
    }

    #[test]
    fn test_symbols() {
        let c = include_str!("test/symbol.proto");
        let p = ProtoFile::new("file://test/symbol.proto".parse().unwrap(), c);

        assert!(p.is_ok());
        assert_yaml_snapshot!(p.unwrap().symbols());
    }
}
