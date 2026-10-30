use std::sync::Arc;

use async_lsp::lsp_types::DidOpenTextDocumentParams;

use crate::server::state::DocumentVersion;
use crate::server::state::cache::DocumentsGuardExt;
use crate::server::state::source::SourceEntry;
use crate::server::state::unique_uris::UniqueUris;

use super::{Notification, Worker};

impl Worker {
    pub(super) async fn open_document(&mut self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let content = Arc::new(params.text_document.text);

        {
            let (mut documents, mut sources) = self.state.write_lock_all().await;

            documents.set_dirty(&uri);

            sources.insert(
                uri.clone(),
                SourceEntry {
                    version: DocumentVersion(version),
                    content,
                },
            );
        }

        let cancel_token = self.shutdown_token.child_token();

        let _ = self
            .state
            .query_documents_batch(UniqueUris::new(uri), cancel_token, None, false)
            .await;
    }
}

impl From<DidOpenTextDocumentParams> for Notification {
    fn from(params: DidOpenTextDocumentParams) -> Self {
        Notification::OpenDocument { params }
    }
}
