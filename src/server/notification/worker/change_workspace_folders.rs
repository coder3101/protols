use std::path::PathBuf;

use async_lsp::lsp_types::DidChangeWorkspaceFoldersParams;
use async_lsp::lsp_types::notification::{DidChangeWorkspaceFolders, Notification as N};

use super::{Notification, Worker};
use crate::config::configure_workspace_folders;
use crate::server::state::ProtoLanguageState;
use crate::utils::OutermostPaths;

impl Worker {
    pub(super) async fn change_workspace_folders(
        &mut self,
        params: DidChangeWorkspaceFoldersParams,
    ) {
        if params.event.added.is_empty() && params.event.removed.is_empty() {
            tracing::warn!(
                method = DidChangeWorkspaceFolders::METHOD,
                "Received change notification with empty content changes"
            );

            return;
        }

        self.indexing.cancel_token.cancel();
        if let Some(finalized) = self.indexing.finalized.take() {
            let _ = finalized.await;
        }

        let configured_folders = configure_workspace_folders(&params.event.added).await;

        {
            let mut configs = self.configs.write().await;

            for folder in &params.event.removed {
                configs.remove_workspace_folder(folder);
            }

            for folder in configured_folders {
                configs.add_workspace_folder(folder);
            }
        }

        if self.shutdown_token.is_cancelled() {
            return;
        }

        let new_outermost = self.configs.read().await.get_all_indexing_paths();
        let removed_paths: Vec<_> = params
            .event
            .removed
            .iter()
            .filter_map(|folder| folder.uri.to_file_path().ok())
            .collect();

        if !removed_paths.is_empty() {
            purge_removed_workspace_folders(&self.state, &removed_paths, &new_outermost).await;
        }

        self.refresh_file_watchers(&new_outermost).await;

        self.run_indexing_pipeline(new_outermost).await;
    }
}

async fn purge_removed_workspace_folders(
    state: &ProtoLanguageState,
    removed_folders: &[PathBuf],
    new_active_scope: &OutermostPaths,
) {
    if removed_folders.is_empty() {
        return;
    }

    {
        let (mut documents, mut sources) = state.write_lock_all().await;

        documents.retain(|url, entry| {
            let Ok(file_path) = url.to_file_path() else {
                return true;
            };

            let inside_removed_folder = removed_folders
                .iter()
                .any(|removed| file_path.starts_with(removed));

            if !inside_removed_folder {
                return true;
            }

            let still_needed_by_include_path = new_active_scope
                .iter()
                .any(|active_outer| file_path.starts_with(active_outer));

            if still_needed_by_include_path {
                return true;
            }

            entry.cancel();

            false
        });

        sources.retain(|url, _| {
            let Ok(file_path) = url.to_file_path() else {
                return true;
            };

            let inside_removed_folder = removed_folders
                .iter()
                .any(|removed| file_path.starts_with(removed));

            if !inside_removed_folder {
                return true;
            }

            new_active_scope
                .iter()
                .any(|active_outer| file_path.starts_with(active_outer))
        });
    }
}

impl From<DidChangeWorkspaceFoldersParams> for Notification {
    fn from(params: DidChangeWorkspaceFoldersParams) -> Self {
        Notification::ChangeWorkspaceFolders { params }
    }
}
