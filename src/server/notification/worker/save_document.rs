use async_lsp::lsp_types::DidSaveTextDocumentParams;

use super::{Notification, Worker};

impl Worker {
    pub(super) async fn save_document(&mut self, params: DidSaveTextDocumentParams) {
        todo!("Add protoc diagnostic update");
    }
}

impl From<DidSaveTextDocumentParams> for Notification {
    fn from(params: DidSaveTextDocumentParams) -> Self {
        Notification::SaveDocument { params }
    }
}
