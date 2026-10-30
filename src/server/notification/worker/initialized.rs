use async_lsp::lsp_types::InitializedParams;

use super::{Notification, Worker};

impl Worker {
    pub(super) async fn initialized(&mut self, _params: InitializedParams) {
        let paths = self.configs.read().await.get_all_indexing_paths();

        if paths.is_empty() {
            tracing::info!("No indexing paths found. Skipping initial workspace indexing.");
            return;
        }

        self.refresh_file_watchers(&paths).await;

        self.run_indexing_pipeline(paths).await;
    }
}

impl From<InitializedParams> for Notification {
    fn from(params: InitializedParams) -> Self {
        Notification::Initialized { params }
    }
}
