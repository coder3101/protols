use async_lsp::lsp_types::{Position, Url};

pub struct HoverContext<'b> {
    pub uri: &'b Url,
    pub position: &'b Position,
    pub imports: Vec<&'b Url>,
}
