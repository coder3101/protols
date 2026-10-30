use async_lsp::lsp_types::notification::{DidChangeWatchedFiles, Notification as N};
use async_lsp::lsp_types::{DidChangeWatchedFilesParams, FileChangeType, FileEvent};

use super::{Notification, Worker};
use crate::server::state::cache::DocumentsGuardExt;

impl Worker {
    pub(super) async fn change_watched_files(&mut self, params: DidChangeWatchedFilesParams) {
        if params.changes.is_empty() {
            tracing::info!(
                method = DidChangeWatchedFiles::METHOD,
                "Received the notification with empty changes list"
            );

            return;
        }

        {
            let (mut documents, mut sources) = self.state.write_lock_all().await;

            for FileEvent { typ, uri } in params.changes {
                match typ {
                    FileChangeType::CREATED => {
                        if sources.remove(&uri).is_some() {
                            tracing::error!(
                                method = DidChangeWatchedFiles::METHOD,
                                %uri,
                                "CRITICAL Cache anomaly: received CREATED event for a URI that already has active content in sources"
                            );
                        }
                        documents.set_dirty(&uri);
                    }

                    FileChangeType::CHANGED => {
                        if sources.contains_key(&uri) {
                            continue;
                        }

                        documents.set_dirty(&uri);
                    }

                    FileChangeType::DELETED => {
                        documents.cancel_and_remove(&uri);

                        if sources.remove(&uri).is_some() {
                            tracing::warn!(
                                method = DidChangeWatchedFiles::METHOD,
                                %uri,
                                "Cache anomaly: watched file was DELETED on disk while still active in sources"
                            );
                        }
                    }
                    change_type => {
                        tracing::warn!(
                            method = DidChangeWatchedFiles::METHOD,
                            ?change_type,
                            %uri,
                            "Unexpecte file change type"
                        );
                    }
                }
            }
        }
    }
}

impl From<DidChangeWatchedFilesParams> for Notification {
    fn from(params: DidChangeWatchedFilesParams) -> Self {
        Notification::ChangeWatchedFiles { params }
    }
}
