use async_lsp::lsp_types::Url;
use tokio_util::sync::CancellationToken;
use walkdir::{DirEntry, WalkDir};

use crate::{
    server::progress::{LspProgressReporter, OptionReporterExt},
    state::unique_uris::UniqueUris,
    utils::OutermostPaths,
};

impl super::ProtoLanguageState {
    pub async fn index_folders(
        &self,
        paths: OutermostPaths,
        cancel_token: CancellationToken,
        progress_reporter: Option<LspProgressReporter>,
    ) {
        if paths.is_empty() {
            return;
        }

        let cancel_token_clone = cancel_token.clone();

        let uris: Vec<_> = tokio::task::spawn_blocking(move || {
            let trace_walkdir_error = |item: &Result<DirEntry, walkdir::Error>| {
                if let Err(err) = item {
                    tracing::warn!("Failed to access path during .proto files scan: {err}");
                }
            };

            paths
                .into_iter()
                .take_while(|_| !cancel_token.is_cancelled())
                .filter(|p| p.is_dir())
                .flat_map(|dir| {
                    WalkDir::new(dir)
                        .follow_links(false)
                        .into_iter()
                        .filter_entry(|e| !cancel_token.is_cancelled() && !is_hidden(e))
                        .inspect(trace_walkdir_error)
                        .filter_map(Result::ok)
                })
                .take_while(|_| !cancel_token.is_cancelled())
                .filter(|de| de.path().extension().is_some_and(|ext| ext == "proto"))
                .filter_map(|de| Url::from_file_path(de.path()).ok())
                .collect()
        })
        .await
        .unwrap_or_default();

        progress_reporter.report(20, "Scan completed. Starting file parsing...");

        if !uris.is_empty() && !cancel_token_clone.is_cancelled() {
            tracing::info!(
                "Workspace scan completed. Booking cache for {} .proto files...",
                uris.len()
            );

            let (finalized, _) = self
                .query_documents_batch(
                    UniqueUris::new(uris),
                    cancel_token_clone,
                    progress_reporter.with_range(20, 100),
                    false,
                )
                .await;

            finalized.await;
        }
    }
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().as_encoded_bytes().starts_with(b".")
}
