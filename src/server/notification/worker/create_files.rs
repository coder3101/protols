use std::str::FromStr;

use async_lsp::lsp_types::notification::{DidCreateFiles, Notification as N};
use async_lsp::lsp_types::{CreateFilesParams, Url};

use crate::server::state::cache::DocumentsGuardExt;

use super::{Notification, Worker};

impl Worker {
    pub(super) async fn create_files(&mut self, params: CreateFilesParams) {
        let uris: Vec<_> = params
            .files
            .iter()
            .filter_map(|f| Url::from_str(f.uri.as_str()).ok())
            .collect();

        if uris.is_empty() {
            tracing::info!(
                method = DidCreateFiles::METHOD,
                "Received create notification with empty files list"
            );

            return;
        }

        {
            let (mut documents, mut sources) = self.state.write_lock_all().await;

            for uri in uris {
                if sources.remove(&uri).is_some() {
                    tracing::error!(
                        method = DidCreateFiles::METHOD,
                        %uri,
                        "CRITICAL cache anomaly: received create notification for a URI that already has active content in sources!"
                    );
                }

                documents.set_dirty(&uri);
            }
        }
    }
}

impl From<CreateFilesParams> for Notification {
    fn from(params: CreateFilesParams) -> Self {
        Notification::CreateFiles { params }
    }
}
