use std::sync::Arc;

use async_lsp::ClientSocket;
use async_lsp::lsp_types::ProgressToken;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::super::progress::{LspProgressChannel, OptionChannelExt};
use super::Notification;
use crate::config::WorkspaceProtoConfigs;
use crate::log::LogReloadHandle;
use crate::state::{FinalizerToken, ProtoLanguageState};
use crate::utils::OutermostPaths;

mod change_document;
mod change_watched_files;
mod change_workspace_folders;
mod close_document;
mod create_files;
mod delete_files;
mod exit;
mod file_watchers;
mod initialized;
mod open_document;
mod rename_files;
mod save_document;
mod set_trace;

struct Indexing {
    cancel_token: CancellationToken,
    finalized: Option<FinalizerToken>,
}

pub struct Worker {
    state: ProtoLanguageState,
    configs: Arc<RwLock<WorkspaceProtoConfigs>>,
    client: ClientSocket,
    log_handle: LogReloadHandle,
    shutdown_token: CancellationToken,
    indexing: Indexing,
}

pub(super) const WORKER_MESSAGE_BUFFER: usize = 256;

impl Worker {
    pub fn start(
        state: ProtoLanguageState,
        configs: Arc<RwLock<WorkspaceProtoConfigs>>,
        log_handle: LogReloadHandle,
        client: ClientSocket,
        shutdown_token: CancellationToken,
    ) -> tokio::sync::mpsc::Sender<Notification> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(WORKER_MESSAGE_BUFFER);

        let indexing = Indexing {
            cancel_token: shutdown_token.child_token(),
            finalized: None,
        };
        let mut worker = Self {
            state,
            configs,
            client,
            log_handle,
            shutdown_token,
            indexing,
        };

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if worker.shutdown_token.is_cancelled()
                    && !matches!(event, Notification::Exit { .. })
                {
                    tracing::info!(
                        %event,
                        "Skipping notification handling because shutdown is in progress."
                    );
                    continue;
                }

                match event {
                    Notification::Initialized { params } => worker.initialized(params).await,
                    Notification::OpenDocument { params } => worker.open_document(params).await,
                    Notification::ChangeDocument { params } => worker.change_document(params).await,
                    Notification::SaveDocument { params } => worker.save_document(params).await,
                    Notification::CloseDocument { params } => worker.close_document(params).await,
                    Notification::CreateFiles { params } => worker.create_files(params).await,
                    Notification::RenameFiles { params } => worker.rename_files(params).await,
                    Notification::DeleteFiles { params } => worker.delete_files(params).await,
                    Notification::ChangeWorkspaceFolders { params } => {
                        worker.change_workspace_folders(params).await;
                    }
                    Notification::ChangeWatchedFiles { params } => {
                        worker.change_watched_files(params).await;
                    }
                    Notification::SetTrace { params } => worker.set_trace(params),
                    Notification::Exit => {
                        worker.exit();
                        break;
                    }
                }
            }
        });

        tx
    }

    async fn run_indexing_pipeline(&mut self, paths: OutermostPaths) {
        self.indexing.cancel_token.cancel();
        if let Some(finalizer) = self.indexing.finalized.take() {
            tracing::warn!(
                "The indexing wasn't cancelled before running the new one. Cancelling..."
            );
            let _ = finalizer.await;
        }

        self.indexing.cancel_token = self.shutdown_token.child_token();
        self.state.set_indexing(true);

        let state = self.state.clone();
        let client = self.client.clone();
        let cancel_token_clone = self.indexing.cancel_token.clone();
        let progress_token = ProgressToken::String("protobuf-workspace-indexing".to_string());

        let task_handle = tokio::spawn(async move {
            let capabilities = state.client_capabilities();
            let channel =
                LspProgressChannel::try_create(capabilities, client, progress_token, false).await;

            channel.begin("Indexing workspace");

            state
                .index_folders(paths, cancel_token_clone, channel.reporter())
                .await;

            channel.end();

            state.set_indexing(false);
        });

        self.indexing.finalized = Some(FinalizerToken::from_handle(task_handle));
    }
}
