use std::sync::Arc;

use async_lsp::lsp_types::DidChangeTextDocumentParams;
use async_lsp::lsp_types::notification::{DidChangeTextDocument, Notification as N};

use super::{Notification, Worker};
use crate::server::state::DocumentVersion;
use crate::server::state::cache::DocumentsGuardExt;
use crate::server::state::source::SourceEntry;

impl Worker {
    pub(super) async fn change_document(&mut self, params: DidChangeTextDocumentParams) {
        let Some(change) = params.content_changes.into_iter().next() else {
            tracing::warn!(
                method = DidChangeTextDocument::METHOD,
                uri = %params.text_document.uri,
                version = params.text_document.version,
                "Received change notification with empty content changes"
            );

            return;
        };

        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let content = Arc::new(change.text);

        {
            let (mut documents, mut sources) = self.state.write_lock_all().await;

            if !documents.set_dirty(&uri) {
                tracing::warn!(
                    method = DidChangeTextDocument::METHOD,
                    %uri,
                    %version,
                    "Cache anomaly: received change notification for a document that is missing"
                );
            }

            let source = sources.get_mut(&uri);

            if let Some(existing_source) = source {
                existing_source.version = DocumentVersion(version);
                existing_source.content = content;
            } else {
                tracing::error!(
                    method = DidChangeTextDocument::METHOD,
                    %uri,
                    %version,
                    "Cache anomaly: received change notification for a document that is missing from sources"
                );

                sources.insert(
                    uri,
                    SourceEntry {
                        version: DocumentVersion(version),
                        content,
                    },
                );
            }
        }
    }
}

impl From<DidChangeTextDocumentParams> for Notification {
    fn from(params: DidChangeTextDocumentParams) -> Self {
        Notification::ChangeDocument { params }
    }
}
