use std::str::FromStr;

use async_lsp::lsp_types::notification::{DidDeleteFiles, Notification as N};
use async_lsp::lsp_types::{DeleteFilesParams, Url};

use crate::server::state::cache::DocumentsGuardExt;

use super::{Notification, Worker};

impl Worker {
    pub(super) async fn delete_files(&mut self, params: DeleteFilesParams) {
        let uris: Vec<_> = params
            .files
            .iter()
            .filter_map(|f| Url::from_str(f.uri.as_str()).ok())
            .collect();

        if uris.is_empty() {
            tracing::info!(
                method = DidDeleteFiles::METHOD,
                "Received delete notification with empty files list"
            );

            return;
        }

        {
            let (mut documents, mut sources) = self.state.write_lock_all().await;

            for uri in uris {
                if sources.remove(&uri).is_some() {
                    tracing::warn!(
                        method = DidDeleteFiles::METHOD,
                        %uri,
                        "Cache anomaly: received delete notification for a document that was still active in sources"
                    );
                }

                documents.cancel_and_remove(&uri);
            }
        }
    }
}

impl From<DeleteFilesParams> for Notification {
    fn from(params: DeleteFilesParams) -> Self {
        Notification::DeleteFiles { params }
    }
}
