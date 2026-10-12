use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock, atomic::AtomicBool};

use async_lsp::lsp_types::Url;
use tokio::sync::RwLock;
use tree_sitter::{Query, QueryError};

use crate::config::client::ClientCapabilitiesSummary;
use crate::model::generate_metamodel_query;
use cache::CacheEntry;
use source::SourceEntry;

pub(super) mod cache;
pub(super) mod error;
pub(super) mod finalizer;
// mod hover;
pub(super) mod index;
mod query;
// mod resolve;
pub(super) mod source;
pub(super) mod unique_uris;

#[derive(Clone)]
pub(crate) struct ProtoLanguageState {
    client_capabilities: Arc<OnceLock<ClientCapabilitiesSummary>>,
    sources: Arc<RwLock<HashMap<Url, SourceEntry>>>,
    documents: Arc<RwLock<HashMap<Url, CacheEntry>>>,
    metamodel_query: Arc<Query>,
    is_indexing: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocumentVersion(pub i32);

impl DocumentVersion {
    pub const DISK: Self = DocumentVersion(-1);
}

impl ProtoLanguageState {
    pub fn new() -> Self {
        let language: tree_sitter::Language = tree_sitter_proto::LANGUAGE.into();
        let trace_error = |e: &QueryError| {
            tracing::error!(
                "Critical SCM error: Failed to compile embedded Tree-sitter query for metadata extraction. Details: {:?}",
                e
            );
        };

        let metamodel_query = Query::new(&language, &generate_metamodel_query())
            .inspect_err(trace_error)
            .expect("Tree-sitter query compilation failed");

        Self {
            client_capabilities: Arc::default(),
            sources: Arc::default(),
            documents: Arc::default(),
            metamodel_query: Arc::new(metamodel_query),
            is_indexing: Arc::default(),
        }
    }

    pub async fn write_lock_all(
        &self,
    ) -> (
        tokio::sync::RwLockWriteGuard<'_, HashMap<Url, CacheEntry>>,
        tokio::sync::RwLockWriteGuard<'_, HashMap<Url, SourceEntry>>,
    ) {
        let documents_guard = self.documents.write().await;
        let sources_guard = self.sources.write().await;

        (documents_guard, sources_guard)
    }

    #[inline]
    pub fn set_client_capabilities(&self, capabilities: ClientCapabilitiesSummary) {
        self.client_capabilities
            .set(capabilities)
            .expect("Attempted to initialize client capabilities more than once per session!");
    }

    #[inline]
    pub fn client_capabilities(&self) -> ClientCapabilitiesSummary {
        self.client_capabilities.get().copied().unwrap_or_default()
    }

    #[inline]
    pub fn set_indexing(&self, value: bool) {
        self.is_indexing.store(value, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_indexing(&self) -> bool {
        self.is_indexing.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub async fn add_dirty_file_for_test(
        &self,
        uri: &Url,
        content: &str,
        version: DocumentVersion,
    ) {
        let content = Arc::new(content.to_owned());

        {
            use crate::server::state::cache::DocumentsGuardExt;

            let mut documents = self.documents.write().await;
            let mut sources = self.sources.write().await;

            documents.set_dirty(uri);
            sources.insert(uri.clone(), SourceEntry { version, content });
        }
    }
}
