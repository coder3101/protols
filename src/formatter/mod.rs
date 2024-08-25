use async_lsp::lsp_types::{Range, TextEdit, Url};

pub mod clang;

pub trait ProtoFormatter: Sized {
    fn format_document(&self, u: &Url) -> Option<Vec<TextEdit>>;
    fn format_document_range(&self, u: &Url, r: &Range) -> Option<Vec<TextEdit>>;
}
