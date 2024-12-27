use async_lsp::lsp_types::{Range, TextEdit};

pub mod clang;

pub trait ProtoFormatter: Sized {
    fn format_document(&self, filename: &str, content: &str) -> Option<Vec<TextEdit>>;
    fn format_document_range(&self, r: &Range, filename: &str, content: &str) -> Option<Vec<TextEdit>>;
}
