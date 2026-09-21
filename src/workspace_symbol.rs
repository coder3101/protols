use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::io::Error;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use async_lsp::lsp_types::Url;
use futures::stream::{self, StreamExt};
use tokio_util::sync::CancellationToken;
use tree_sitter::{Language, Parser, Query};
use walkdir::{DirEntry, WalkDir};

use crate::document::ProtoDocument;
use crate::state::{CacheEntry, DocumentVersion};

const MAX_CONCURRENT_DISK_SCANS: usize = 2;
const MAX_WARMUP_FILE_SIZE_BYTES: u64 = 1024 * 1024;

/// # Cancellation safety
///
/// This method is cancel safe.
pub async fn warmup_workspaces(
    paths: HashSet<PathBuf>,
    query: Arc<Query>,
    cache: Arc<RwLock<HashMap<Url, CacheEntry>>>,
    cancel_token: CancellationToken,
) {
    let () = stream::iter(paths)
        .map(|path| {
            find_and_parse_proto_files(path, query.clone(), cache.clone(), cancel_token.clone())
        })
        .buffer_unordered(MAX_CONCURRENT_DISK_SCANS)
        .collect()
        .await;
}

async fn find_and_parse_proto_files(
    workspace_path: PathBuf,
    query: Arc<Query>,
    cache: Arc<RwLock<HashMap<Url, CacheEntry>>>,
    cancel_token: CancellationToken,
) {
    if !workspace_path.is_dir() {
        return;
    }

    tokio::task::spawn_blocking(move || {
        let trace_walkdir_error = |item: &Result<DirEntry, walkdir::Error>| {
            if let Err(err) = item {
                tracing::warn!("Failed to access path during .proto files scan: {err}");
            }
        };

        let entries = WalkDir::new(workspace_path)
            .max_depth(8)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !cancel_token.is_cancelled() && !is_hidden(e))
            .inspect(trace_walkdir_error)
            .filter_map(Result::ok)
            .take_while(|_| !cancel_token.is_cancelled())
            .filter(|de| {
                de.metadata()
                    .is_ok_and(|m| m.is_file() && m.len() <= MAX_WARMUP_FILE_SIZE_BYTES)
                    && de.path().extension().is_some_and(|ext| ext == "proto")
            });

        let mut parser = Parser::new();
        let language: Language = tree_sitter_proto::LANGUAGE.into();

        if parser.set_language(&language).is_err() {
            tracing::error!("Critical: Failed to set proto language for Tree-sitter parser.");
            return;
        }

        let local_documents: HashMap<Url, ProtoDocument> = entries
            .take_while(|_| !cancel_token.is_cancelled())
            .filter_map(|entry| {
                let path = entry.path();

                let uri = Url::from_file_path(path).ok()?;

                let trace_read_error =
                    |e: &Error| tracing::error!("Warmup: Failed to read file {:?}: {}", path, e);

                let source = std::fs::read(path).inspect_err(trace_read_error).ok()?;

                if cancel_token.is_cancelled() {
                    return None;
                }

                let document =
                    ProtoDocument::try_from_input(uri.clone(), &source, &query, &mut parser)?;

                tracing::info!(" -> Successfully warmed up: {}", uri.as_str());
                Some((uri, document))
            })
            .collect();

        if local_documents.is_empty() {
            return;
        }

        let mut cache_guard = cache.write().expect("poison");

        tracing::info!(
            "Eager Warmup: Atomic integration of {} document(s) into state cache.",
            local_documents.len()
        );

        for (uri, document) in local_documents {
            if let Entry::Vacant(entry) = cache_guard.entry(uri) {
                entry.insert(CacheEntry::Ready {
                    version: DocumentVersion(0),
                    document,
                });
            }
        }
    })
    .await
    .unwrap_or_default();
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name().as_encoded_bytes().starts_with(b".")
}
