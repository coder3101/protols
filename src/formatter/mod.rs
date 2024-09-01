use async_lsp::lsp_types::{Range, TextEdit};

pub mod clang;

pub trait ProtoFormatter: Sized {
    fn format_document(&self, content: &str) -> Option<Vec<TextEdit>>;
    fn format_document_range(&self, r: &Range, content: &str) -> Option<Vec<TextEdit>>;
}
