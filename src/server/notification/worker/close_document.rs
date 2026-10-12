use async_lsp::lsp_types::DidCloseTextDocumentParams;
use async_lsp::lsp_types::notification::{DidCloseTextDocument, Notification as N};

use crate::server::state::cache::DocumentsGuardExt;

use super::{Notification, Worker};

impl Worker {
    pub(super) async fn close_document(&mut self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        {
            let (mut documents, mut sources) = self.state.write_lock_all().await;

            if !documents.set_dirty(&uri) {
                tracing::warn!(
                    method = DidCloseTextDocument::METHOD,
                    %uri,
                    "Cache anomaly: received close notification for a document that is missing"
                );
            }

            if sources.remove(&uri).is_none() {
                tracing::error!(
                    method = DidCloseTextDocument::METHOD,
                    %uri,
                    "Cache anomaly: received close notification for a document that was missing from sources"
                );
            }
        }
    }
}

impl From<DidCloseTextDocumentParams> for Notification {
    fn from(params: DidCloseTextDocumentParams) -> Self {
        Notification::CloseDocument { params }
    }
}
