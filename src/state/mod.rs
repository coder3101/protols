mod definition;
mod hover;
mod index;
mod rename;
mod resolve;
pub(super) mod unique_uris;
mod workspace_symbol;

use futures::Stream;
use futures::future::{BoxFuture, Either, FutureExt, Shared};
use futures::stream::{FuturesUnordered, StreamExt};
use std::pin::Pin;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc::Sender},
};
use tokio_util::sync::CancellationToken;
use tracing::info;

use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, FileDelete, Location, OneOf, ProgressParamsValue,
    PublishDiagnosticsParams, Range, SymbolKind, SymbolTag, Url, WorkspaceSymbol,
};
use tokio::sync::{RwLock, RwLockWriteGuard, oneshot};
use tree_sitter::{LanguageError, Query, QueryError};
use walkdir::WalkDir;

use crate::config::client::ClientCapabilitiesSummary;
use crate::server::progress::{LspProgressReporter, OptionReporterExt};
use crate::state::unique_uris::UniqueUris;
use crate::{
    config::Config,
    document::{ProtoDocument, ProtoParser},
    model::{ElementKind, generate_metamodel_query},
    protoc::collect_diagnostics,
};

const MAX_PROTO_FILE_SIZE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    DocumentNotFound { uri: Url },
    SourceNotFound { uri: Url },
    InvalidPath { uri: Url },
    InvalidExtension { uri: Url },
    FileTooLarge { uri: Url, size: u64 },
    DiskIoError { uri: Url, details: String },
    ParserError { uri: Url },
    RequestCancelled { uri: Url },
    InternalError { uri: Url },
}

impl From<CoreError> for async_lsp::ResponseError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::DocumentNotFound { uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::INVALID_PARAMS,
                format!("Protols cache failure: Document metadata wrapper not found {uri}"),
            ),

            CoreError::SourceNotFound { uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::INTERNAL_ERROR,
                format!("Protols cache corruption: Raw source text missing for tracked file {uri}"),
            ),

            CoreError::InvalidPath { uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::INTERNAL_ERROR,
                format!("Protols failed to convert URI to OS path: {uri}"),
            ),

            CoreError::InvalidExtension { uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::REQUEST_FAILED,
                format!("Unsupported file extension. Protols only handles '.proto' files: {uri}"),
            ),

            CoreError::FileTooLarge { uri, size } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::REQUEST_FAILED,
                format!(
                    "File exceeds maximum allowed size for parsing ({} bytes): {uri}",
                    size
                ),
            ),

            CoreError::DiskIoError { details, uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::INTERNAL_ERROR,
                format!("Protols failed to read from disk: {uri}. Details: {details}"),
            ),

            CoreError::ParserError { uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::INTERNAL_ERROR,
                format!("Protols failed to create parser for URI {uri}"),
            ),

            CoreError::RequestCancelled { uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::REQUEST_CANCELLED,
                format!("LSP request was aborted or superseded by a newer action for URI {uri}"),
            ),

            CoreError::InternalError { uri } => async_lsp::ResponseError::new(
                async_lsp::ErrorCode::INTERNAL_ERROR,
                format!(
                    "Protols internal panic: Batch processing engine lost requested file map entry {uri}"
                ),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocumentVersion(pub i32);

impl DocumentVersion {
    pub const DISK: Self = DocumentVersion(-1);
}

#[derive(Clone)]
pub enum CacheEntry {
    Ready {
        version: DocumentVersion,
        result: Result<Arc<ProtoDocument>, CoreError>,
    },
    Pending {
        version: DocumentVersion,
        future: Shared<BoxFuture<'static, Result<Arc<ProtoDocument>, CoreError>>>,
        cancel_token: CancellationToken,
    },
    Dirty,
}

impl CacheEntry {
    fn is_transient_error(result: &Result<Arc<ProtoDocument>, CoreError>) -> bool {
        match result {
            Ok(_)
            | Err(
                CoreError::DiskIoError { .. }
                | CoreError::InvalidPath { .. }
                | CoreError::InvalidExtension { .. }
                | CoreError::SourceNotFound { .. }
                | CoreError::InternalError { .. },
            ) => false,
            Err(
                CoreError::DocumentNotFound { .. }
                | CoreError::ParserError { .. }
                | CoreError::RequestCancelled { .. }
                | CoreError::FileTooLarge { .. },
            ) => true,
        }
    }

    pub fn finalize(
        &mut self,
        version: DocumentVersion,
        parse_result: Result<Arc<ProtoDocument>, CoreError>,
    ) {
        if let Self::Pending {
            version: current_version,
            ..
        } = self
            && version == *current_version
        {
            *self = Self::Ready {
                version,
                result: parse_result,
            };
        }
    }

    #[inline]
    pub fn cancel(&self) {
        if let Self::Pending { cancel_token, .. } = self {
            cancel_token.cancel();
        }
    }
}

pub trait DocumentsGuardExt {
    /// Forces the document state to `Dirty`, cancels any pending background tasks,
    /// and returns `true` if the document already existed in the cache.
    fn set_dirty(&mut self, uri: &Url) -> bool;

    /// Completely removes the document from the cache, triggers `cancel()` if it was `Pending`,
    /// and returns `Some(CacheEntry)` if the document existed.
    fn cancel_and_remove(&mut self, uri: &Url) -> Option<CacheEntry>;
}

impl DocumentsGuardExt for RwLockWriteGuard<'_, HashMap<Url, CacheEntry>> {
    fn set_dirty(&mut self, uri: &Url) -> bool {
        if let Some(entry) = self.get_mut(uri) {
            entry.cancel();
            *entry = CacheEntry::Dirty;

            return true;
        }

        self.insert(uri.clone(), CacheEntry::Dirty).is_some()
    }

    fn cancel_and_remove(&mut self, uri: &Url) -> Option<CacheEntry> {
        if let Some(entry) = self.remove(uri) {
            entry.cancel();
            Some(entry)
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct SourceEntry {
    pub version: DocumentVersion,
    pub content: Arc<String>,
}

#[derive(Clone)]
pub struct ProtoLanguageState {
    client_capabilities: OnceLock<ClientCapabilitiesSummary>,
    pub(super) sources: Arc<RwLock<HashMap<Url, SourceEntry>>>,
    pub(super) documents: Arc<RwLock<HashMap<Url, CacheEntry>>>,
    parser: Arc<Mutex<ProtoParser>>,
    parsed_workspaces: Arc<RwLock<HashSet<String>>>,
    metamodel_query: Arc<Query>,
    is_indexing: Arc<AtomicBool>,
}

const WRITE_CHUNK_SIZE: usize = 16;

enum TaskSource {
    FromMemory(Arc<String>),
    FromDisk,
}

pub struct FinalizerToken {
    inner: tokio::task::JoinHandle<()>,
}

impl FinalizerToken {
    /// Creates a dummy completed finalizer token that resolves instantly.
    pub fn dummy() -> Self {
        Self {
            inner: tokio::spawn(async {}),
        }
    }

    pub(crate) fn from_handle(task_handle: tokio::task::JoinHandle<()>) -> Self {
        Self { inner: task_handle }
    }
}

impl Future for FinalizerToken {
    type Output = Result<(), tokio::task::JoinError>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx)
    }
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
            client_capabilities: OnceLock::new(),
            sources: Arc::default(),
            documents: Arc::default(),
            parser: Arc::new(Mutex::new(ProtoParser::new())),
            parsed_workspaces: Arc::new(RwLock::new(HashSet::new())),
            metamodel_query: Arc::new(metamodel_query),
            is_indexing: Arc::new(AtomicBool::new(false)),
        }
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

    pub fn get_content(&self, uri: &Url) -> String {
        self.sources
            .read()
            .expect("poison")
            .get(uri)
            .map(|e| e.content.clone())
            .unwrap_or_default()
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

    /// Processes a batch of documents.
    ///
    /// # Cancellation safety
    ///
    /// This method is cancel safe.
    pub async fn query_documents_batch(
        &self,
        uris: UniqueUris,
        batch_cancel_token: CancellationToken,
        progress_reporter: Option<LspProgressReporter>,
        need_results: bool,
    ) -> (
        FinalizerToken,
        impl Stream<Item = Result<Arc<ProtoDocument>, CoreError>>,
    ) {
        let mut cache_hits = Vec::new();
        let mut client_futures = FuturesUnordered::new();
        let mut needs_parsing = false;

        {
            let documents = self.documents.read().await;

            for uri in &uris {
                match documents.get(uri) {
                    Some(CacheEntry::Ready { result, .. })
                        if !CacheEntry::is_transient_error(result) =>
                    {
                        cache_hits.extend(need_results.then(|| result.clone()));
                    }
                    Some(CacheEntry::Pending { future, .. }) => {
                        client_futures.extend(need_results.then(|| future.clone()));
                    }
                    _ => {
                        needs_parsing = true;
                        break;
                    }
                }
            }
        }

        if !needs_parsing {
            return (
                FinalizerToken::dummy(),
                Either::Left(futures::stream::iter(cache_hits).chain(client_futures)),
            );
        }

        cache_hits.clear();
        client_futures.clear();

        let finalizer_futures = FuturesUnordered::new();
        let mut tasks = Vec::new();

        {
            let mut documents = self.documents.write().await;
            let sources = self.sources.read().await;

            for uri in uris {
                let entry = documents.get(&uri);

                match entry {
                    Some(CacheEntry::Ready { result, .. })
                        if !CacheEntry::is_transient_error(result) =>
                    {
                        cache_hits.extend(need_results.then(|| result.clone()));
                        continue;
                    }
                    Some(CacheEntry::Pending { future, .. }) => {
                        client_futures.extend(need_results.then(|| future.clone()));
                        continue;
                    }
                    _ => {}
                }

                let source = sources.get(&uri);

                if source.is_none()
                    && matches!(
                        entry,
                        Some(CacheEntry::Dirty) | Some(CacheEntry::Ready { .. })
                    )
                {
                    let result = Err(CoreError::SourceNotFound { uri: uri.clone() });

                    cache_hits.extend(need_results.then(|| result.clone()));
                    documents.insert(
                        uri.clone(),
                        CacheEntry::Ready {
                            version: DocumentVersion::DISK,
                            result,
                        },
                    );

                    continue;
                }

                let (expected_version, task_source) = source.map_or_else(
                    || (DocumentVersion::DISK, TaskSource::FromDisk),
                    |se| (se.version, TaskSource::FromMemory(se.content.clone())),
                );

                let (tx, rx) = oneshot::channel();
                let cancel_token = batch_cancel_token.child_token();
                let cancel_token_clone = cancel_token.clone();
                let uri_clone = uri.clone();

                let future = async move {
                    let uri = uri_clone;
                    cancel_token_clone
                        .run_until_cancelled(rx)
                        .await
                        .map(|res| {
                            res.unwrap_or(Err(CoreError::RequestCancelled { uri: uri.clone() }))
                        })
                        .unwrap_or(Err(CoreError::RequestCancelled { uri }))
                }
                .boxed()
                .shared();

                let version = expected_version;

                finalizer_futures
                    .push(future.clone().map(move |result| (expected_version, result)));

                client_futures.extend(need_results.then(|| future.clone()));

                documents.insert(
                    uri.clone(),
                    CacheEntry::Pending {
                        version,
                        future,
                        cancel_token: cancel_token.clone(),
                    },
                );

                tasks.push((uri, task_source, tx, cancel_token));
            }
        }

        self.run_blocking_parser(tasks);

        (
            self.run_cache_finalizer(finalizer_futures, progress_reporter),
            Either::Right(futures::stream::iter(cache_hits).chain(client_futures)),
        )
    }

    fn run_blocking_parser(
        &self,
        tasks: Vec<(
            Url,
            TaskSource,
            oneshot::Sender<Result<Arc<ProtoDocument>, CoreError>>,
            CancellationToken,
        )>,
    ) {
        if tasks.is_empty() {
            return;
        }

        let query = self.metamodel_query.clone();
        tokio::task::spawn_blocking(move || {
            let mut parser = tree_sitter::Parser::new();

            let trace_error = |error: &LanguageError| {
                tracing::error!(
                    ?error,
                    "Critical initialization failure: Failed to set Tree-sitter Protobuf language parser"
                );
            };

            parser
                .set_language(&tree_sitter_proto::LANGUAGE.into())
                .inspect_err(trace_error)
                .expect("Tree-sitter parser: setting language failed");

            for (uri, task_source, tx, cancel_token) in tasks {
                if cancel_token.is_cancelled() {
                    let _ = tx.send(Err(CoreError::RequestCancelled { uri }));
                    continue;
                }

                let mut disk_buffer = Vec::new();

                if let TaskSource::FromDisk = &task_source {
                    let path = match uri.to_file_path() {
                        Ok(path) => path,
                        Err(_) => {
                            let _ = tx.send(Err(CoreError::InvalidPath { uri }));
                            continue;
                        }
                    };

                    if !path.extension().is_some_and(|ext| ext == "proto") {
                        let _ = tx.send(Err(CoreError::InvalidExtension { uri }));
                        continue;
                    }

                    match std::fs::metadata(&path) {
                        Err(e) => {
                            let _ = tx.send(Err(CoreError::DiskIoError {
                                uri,
                                details: e.to_string(),
                            }));
                            continue;
                        }
                        Ok(metadata) => {
                            let size = metadata.len();

                            if size > MAX_PROTO_FILE_SIZE_BYTES {
                                tracing::warn!(
                                    %uri,
                                    size,
                                    "Refusing to parse file: size exceeds maximum limit of {} bytes",
                                    MAX_PROTO_FILE_SIZE_BYTES
                                );
                                let _ = tx.send(Err(CoreError::FileTooLarge { uri, size }));
                                continue;
                            }

                            if !metadata.is_file() {
                                let _ = tx.send(Err(CoreError::InvalidPath { uri }));
                                continue;
                            }
                        }
                    }

                    match std::fs::read(&path) {
                        Ok(bytes) => {
                            disk_buffer = bytes;
                        }
                        Err(error) => {
                            let _ = tx.send(Err(CoreError::DiskIoError {
                                uri,
                                details: error.to_string(),
                            }));
                            continue;
                        }
                    }
                }

                let bytes_ref: &[u8] = match &task_source {
                    TaskSource::FromMemory(content) => content.as_bytes(),
                    TaskSource::FromDisk => &disk_buffer,
                };

                if cancel_token.is_cancelled() {
                    let _ = tx.send(Err(CoreError::RequestCancelled { uri }));
                    continue;
                }

                let result =
                    ProtoDocument::try_from_input(uri.clone(), bytes_ref, &query, &mut parser)
                        .map(Arc::new)
                        .ok_or(CoreError::ParserError { uri });

                let _ = tx.send(result);
            }
        });
    }

    fn run_cache_finalizer<Fut>(
        &self,
        futures: FuturesUnordered<Fut>,
        progress_reporter: Option<LspProgressReporter>,
    ) -> FinalizerToken
    where
        Fut: Future<Output = (DocumentVersion, Result<Arc<ProtoDocument>, CoreError>)>
            + Send
            + 'static,
    {
        let total_tasks = futures.len();

        if total_tasks == 0 {
            tracing::warn!(
                "Document cache finalizer called with 0 tasks. This might indicate lost futures."
            );

            return FinalizerToken::dummy();
        }

        let mut completed_tasks = 0;
        let mut chunks = futures.ready_chunks(WRITE_CHUNK_SIZE);
        let documents_clone = self.documents.clone();

        let inner = tokio::spawn(async move {
            while let Some(chunk) = chunks.next().await {
                {
                    let mut documents = documents_clone.write().await;

                    for (expected_version, result) in chunk {
                        if let Some(document) = documents.get_mut(get_uri(&result)) {
                            document.finalize(expected_version, result);
                        }

                        completed_tasks += 1;
                    }
                }

                let percentage = (completed_tasks * 100 / total_tasks) as u32;
                progress_reporter.report(
                    percentage,
                    &format!("Parsing [{completed_tasks}/{total_tasks}]"),
                );

                tokio::task::yield_now().await;
            }
        });

        FinalizerToken { inner }
    }

    pub async fn get_document(
        &self,
        uri: &Url,
        cancel_token: CancellationToken,
    ) -> Result<Arc<ProtoDocument>, CoreError> {
        let uris = UniqueUris::new(uri.clone());

        let (_, mut stream) = self
            .query_documents_batch(uris, cancel_token, None, true)
            .await;

        match stream.next().await {
            Some(result) if get_uri(&result) == uri => result,

            Some(result) => {
                let returned_uri = get_uri(&result);
                tracing::error!(
                    ?returned_uri,
                    expected_uri = ?uri,
                    "Bug: query_documents_batch returned a mismatched URL"
                );
                Err(CoreError::InternalError { uri: uri.clone() })
            }

            None => Err(CoreError::DocumentNotFound { uri: uri.clone() }),
        }
    }

    pub fn get_documents(&self) -> Vec<ProtoDocument> {
        self.documents
            .read()
            .expect("poison")
            .values()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn get_documents_for_package(&self, package: &str) -> Vec<ProtoDocument> {
        self.documents
            .read()
            .expect("poison")
            .values()
            .filter(|document| document.package == package)
            .map(ToOwned::to_owned)
            .collect()
    }

    /// Runs a fast, pure-Rust substring match over the cached metamodel pool
    /// populated during startup indexing.
    ///
    /// This deliberately avoids re-parsing the workspace or rebuilding the
    /// hierarchical [`DocumentSymbol`] document on every request. Instead it scans
    /// the flat, already-indexed [`ModelElement`] registry and resolves each
    /// candidate's container name by walking the in-memory parent links.
    pub fn find_workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let query = query.to_lowercase();
        let mut symbols = Vec::new();

        for document in self.get_documents() {
            for element in &document.elements {
                if matches!(element.kind, ElementKind::Import { .. }) {
                    continue;
                }

                let name_lower = element.meta.name.to_lowercase();
                if !query.is_empty() && !name_lower.contains(&query) {
                    continue;
                }

                let container_name = element
                    .parent_id
                    .and_then(|parent_id| document.elements.get(parent_id))
                    .map(|parent| parent.meta.name.clone());

                let range =
                    element
                        .meta
                        .documentation
                        .first()
                        .map_or(element.meta.range, |comment| Range {
                            start: comment.range.start,
                            end: element.meta.range.end,
                        });

                symbols.push(WorkspaceSymbol {
                    name: element.meta.name.clone(),
                    kind: SymbolKind::from(&element.kind),
                    tags: element
                        .kind
                        .is_deprecated()
                        .then(|| vec![SymbolTag::DEPRECATED]),
                    container_name,
                    location: OneOf::Left(Location {
                        uri: document.uri.clone(),
                        range,
                    }),
                    data: None,
                });
            }
        }

        // Sort symbols by name and then by URI for consistent ordering
        symbols.sort_by(|a, b| {
            let name_cmp = a.name.cmp(&b.name);
            if name_cmp != std::cmp::Ordering::Equal {
                return name_cmp;
            }
            // Extract URI from location
            match (&a.location, &b.location) {
                (OneOf::Left(loc_a), OneOf::Left(loc_b)) => {
                    loc_a.uri.as_str().cmp(loc_b.uri.as_str())
                }
                _ => std::cmp::Ordering::Equal,
            }
        });

        symbols
    }

    fn upsert_content_impl(
        &mut self,
        uri: &Url,
        content: &str,
        ipath: &[PathBuf],
        depth: usize,
        parse_session: &mut HashSet<Url>,
    ) {
        // Safety: to not cause stack overflow
        if depth == 0 {
            return;
        }

        // avoid re-parsing same file incase of circular dependencies
        if parse_session.contains(uri) {
            return;
        }

        let Some(parsed) = self.parser.lock().expect("poison").parse(
            uri.clone(),
            content.as_bytes(),
            &self.metamodel_query,
        ) else {
            return;
        };

        self.documents
            .write()
            .expect("posion")
            .insert(uri.clone(), parsed);

        self.sources
            .write()
            .expect("poison")
            .insert(uri.clone(), content.to_string());

        parse_session.insert(uri.clone());
        let imports = self.get_owned_imports(uri, content);

        for import in &imports {
            if let Some(p) = ipath.iter().map(|p| p.join(import)).find(|p| p.exists())
                && let Ok(uri) = Url::from_file_path(p.clone())
                && let Ok(content) = std::fs::read_to_string(p)
            {
                self.upsert_content_impl(&uri, &content, ipath, depth - 1, parse_session);
            }
        }
    }

    fn get_owned_imports(&self, uri: &Url, _content: &str) -> Vec<String> {
        self.get_document(uri)
            .map(|t| t.import_paths())
            .unwrap_or_default()
    }

    pub fn upsert_content(
        &mut self,
        uri: &Url,
        content: &str,
        ipath: &[PathBuf],
        depth: usize,
    ) -> Vec<String> {
        let mut session = HashSet::new();
        self.upsert_content_impl(uri, content, ipath, depth, &mut session);

        // After content is upserted, those imports which couldn't be located
        // are flagged as import error
        self.get_document(uri)
            .map(|t| t.import_paths())
            .unwrap_or_default()
            .into_iter()
            .filter(|import| !ipath.iter().any(|p| p.join(import.as_str()).exists()))
            .collect()
    }

    pub fn parse_all_from_workspace(
        &mut self,
        workspace: &Path,
        progress_sender: Option<&Sender<ProgressParamsValue>>,
    ) {
        if self
            .parsed_workspaces
            .read()
            .expect("poison")
            .contains(workspace.to_str().unwrap_or_default())
        {
            return;
        }

        let files: Vec<_> = WalkDir::new(workspace.to_str().unwrap_or_default())
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                if let Some(ext) = e.path().extension() {
                    return ext == "proto";
                }
                false
            })
            .collect();

        let total_files = files.len();

        for (idx, file) in files.into_iter().enumerate() {
            let path = file.path();
            if path.is_absolute()
                && path.is_file()
                && let Ok(content) = std::fs::read_to_string(path)
                && let Ok(uri) = Url::from_file_path(path)
            {
                if self.documents.read().expect("poison").contains_key(&uri) {
                    continue;
                }
                self.upsert_content(&uri, &content, &[], 1);

                if let Some(sender) = &progress_sender {
                    let percentage =
                        u32::try_from((idx + 1 / total_files) * 100).unwrap_or_default();
                    let _ = sender.send(ProgressParamsValue::WorkDone(
                        async_lsp::lsp_types::WorkDoneProgress::Report(
                            async_lsp::lsp_types::WorkDoneProgressReport {
                                cancellable: None,
                                message: Some(format!(
                                    "Parsing file {} of {}",
                                    idx + 1,
                                    total_files
                                )),
                                percentage: Some(percentage),
                            },
                        ),
                    ));
                }
            }
        }

        self.parsed_workspaces
            .write()
            .expect("poison")
            .insert(workspace.to_str().unwrap_or_default().to_string());
    }

    pub fn upsert_file(
        &mut self,
        uri: &Url,
        content: &str,
        ipath: &[PathBuf],
        depth: usize,
        config: &Config,
        protoc_diagnostics: bool,
    ) -> Option<PublishDiagnosticsParams> {
        info!(%uri, %depth, "upserting file");
        let diag = self.upsert_content(uri, content, ipath, depth);
        let diag_slice: Vec<&str> = diag.iter().map(String::as_str).collect();
        self.get_document(uri).map(|document| {
            let mut d = vec![];
            d.extend(document.collect_parse_diagnostics());
            d.extend(document.collect_import_diagnostics(diag_slice.as_slice()));

            // Add protoc diagnostics if enabled
            if protoc_diagnostics && let Ok(file_path) = uri.to_file_path() {
                let protoc_diags = collect_diagnostics(
                    &config.path.protoc,
                    file_path.to_str().unwrap_or_default(),
                    &ipath
                        .iter()
                        .map(|p| p.to_str().unwrap_or_default().to_string())
                        .collect::<Vec<_>>(),
                );
                d.extend(protoc_diags);
            }

            PublishDiagnosticsParams {
                uri: document.uri.clone(),
                diagnostics: d,
                version: None,
            }
        })
    }

    pub fn delete_file(&mut self, uri: &Url) {
        info!(%uri, "deleting file");
        self.sources.write().expect("poison").remove(uri);
        self.documents.write().expect("poison").remove(uri);
    }

    pub fn completion_items_for_document(&self, url: &Url) -> Vec<CompletionItem> {
        let collector = |f: fn(&ElementKind) -> bool, k: CompletionItemKind| {
            self.get_document(url)
                .map(|document| {
                    document
                        .elements
                        .iter()
                        .filter(|e| f(&e.kind))
                        .map(|e| CompletionItem {
                            label: format!(".{}.{}", document.package, e.meta.name),
                            kind: Some(k),
                            ..Default::default()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };

        let mut result = collector(is_enum_kind, CompletionItemKind::ENUM);
        result.extend(collector(is_message_kind, CompletionItemKind::STRUCT));
        // Better ways to dedup, but who cares?...
        result.sort_by_key(|k| k.label.clone());
        result.dedup_by_key(|k| k.label.clone());
        result
    }

    pub fn completion_items_for_package(&self, package: &str) -> Vec<CompletionItem> {
        let collector =
            |f: fn(&ElementKind) -> bool, k: CompletionItemKind| {
                self.get_documents_for_package(package).into_iter().fold(
                    vec![],
                    |mut v, document| {
                        let t = document.elements.iter().filter(|e| f(&e.kind)).map(|e| {
                            CompletionItem {
                                label: e.meta.name.clone(),
                                kind: Some(k),
                                ..Default::default()
                            }
                        });
                        v.extend(t);
                        v
                    },
                )
            };

        let mut result = collector(is_enum_kind, CompletionItemKind::ENUM);
        result.extend(collector(is_message_kind, CompletionItemKind::STRUCT));
        // Better ways to dedup, but who cares?...
        result.sort_by_key(|k| k.label.clone());
        result.dedup_by_key(|k| k.label.clone());
        result
    }
}

fn is_enum_kind(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::Enum { .. })
}

fn is_message_kind(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::Message { .. })
}

fn get_uri(result: &Result<Arc<ProtoDocument>, CoreError>) -> &Url {
    match result {
        Ok(d) => &d.uri,
        Err(
            CoreError::DiskIoError { uri, .. }
            | CoreError::DocumentNotFound { uri }
            | CoreError::InternalError { uri }
            | CoreError::InvalidPath { uri }
            | CoreError::InvalidExtension { uri }
            | CoreError::FileTooLarge { uri, .. }
            | CoreError::ParserError { uri }
            | CoreError::RequestCancelled { uri }
            | CoreError::SourceNotFound { uri },
        ) => uri,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use async_lsp::lsp_types::Url;
    use std::path::PathBuf;

    fn uri(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn setup_state() -> ProtoLanguageState {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];

        state.upsert_content(
            &uri("file:///test.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book { string title = 1; }\nenum Color { RED = 0; }\n",
            ipath,
            1,
        );
        state.upsert_content(
            &uri("file:///other.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Author { string name = 1; }\n",
            ipath,
            1,
        );
        state.upsert_content(
            &uri("file:///diff.proto"),
            "syntax = \"proto3\";\npackage com.other;\nmessage Foo { int32 bar = 1; }\n",
            ipath,
            1,
        );
        state
    }

    #[test]
    fn test_get_content() {
        let state = setup_state();
        assert_eq!(
            state.get_content(&uri("file:///test.proto")),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book { string title = 1; }\nenum Color { RED = 0; }\n"
        );
        assert_eq!(state.get_content(&uri("file:///nonexistent.proto")), "");
    }

    #[test]
    fn test_get_document() {
        let state = setup_state();
        assert!(state.get_document(&uri("file:///test.proto")).is_some());
        assert!(
            state
                .get_document(&uri("file:///nonexistent.proto"))
                .is_none()
        );
    }

    #[test]
    fn test_get_documents() {
        let state = setup_state();
        let documents = state.get_documents();
        assert_eq!(documents.len(), 3);
    }

    #[test]
    fn test_get_documents_for_package() {
        let state = setup_state();
        let test_documents = state.get_documents_for_package("com.test");
        assert_eq!(test_documents.len(), 2);

        let other_documents = state.get_documents_for_package("com.other");
        assert_eq!(other_documents.len(), 1);

        let empty_documents = state.get_documents_for_package("com.nonexistent");
        assert!(empty_documents.is_empty());
    }

    #[test]
    fn test_document_completion_items() {
        let state = setup_state();
        let items = state.completion_items_for_document(&uri("file:///test.proto"));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&".com.test.Book"));
        assert!(labels.contains(&".com.test.Color"));
        assert!(!labels.contains(&".com.test.Author"));
    }

    #[test]
    fn test_completion_excludes_fields_enum_values_and_imports() {
        // Completion offers only type-level symbols (messages/enums); fields,
        // enum values, and imports must never leak in.
        let state = setup_state();
        let items = state.completion_items_for_document(&uri("file:///test.proto"));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(!labels.contains(&".com.test.Book.title"));
        assert!(!labels.contains(&".com.test.Color.RED"));
        assert!(!labels.contains(&".com.test.import"));
    }

    #[test]
    fn test_document_completion_empty_for_missing_document() {
        let state = setup_state();
        assert!(
            state
                .completion_items_for_document(&uri("file:///missing.proto"))
                .is_empty()
        );
    }

    #[test]
    fn test_package_completion_items() {
        let state = setup_state();
        let items = state.completion_items_for_package("com.test");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Book"));
        assert!(labels.contains(&"Color"));
        assert!(labels.contains(&"Author"));
        assert!(!labels.contains(&"Foo"));

        let other_items = state.completion_items_for_package("com.other");
        let other_labels: Vec<&str> = other_items.iter().map(|i| i.label.as_str()).collect();
        assert!(other_labels.contains(&"Foo"));
        assert!(!other_labels.contains(&"Book"));
    }

    #[test]
    fn test_package_completion_items_empty_package() {
        let state = setup_state();
        let items = state.completion_items_for_package("com.nonexistent");
        assert!(items.is_empty());
    }

    #[test]
    fn test_find_workspace_symbols_empty_query() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Book"));
        assert!(names.contains(&"Author"));
        assert!(names.contains(&"Color"));
        assert!(names.contains(&"Foo"));
    }

    #[test]
    fn test_find_workspace_symbols_partial_query() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("oo");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Book"));
        assert!(names.contains(&"Foo"));
        assert!(!names.contains(&"Author"));
    }

    #[test]
    fn test_find_workspace_symbols_case_insensitive() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("book");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Book"));
    }

    #[test]
    fn test_find_workspace_symbols_no_match() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("zzzzz");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_delete_file() {
        let mut state = setup_state();
        let test_uri = uri("file:///test.proto");
        assert!(state.get_document(&test_uri).is_some());
        state.delete_file(&test_uri);
        assert!(state.get_document(&test_uri).is_none());
        assert_eq!(state.get_content(&test_uri), "");
    }

    // #[test]
    // fn test_rename_file() {
    //     let mut state = setup_state();
    //     let old_uri = uri("file:///test.proto");
    //     let new_uri = uri("file:///renamed.proto");

    //     assert!(state.get_document(&old_uri).is_some());
    //     assert!(state.get_document(&new_uri).is_none());

    //     state.rename_file(&new_uri, &old_uri);

    //     assert!(state.get_document(&old_uri).is_none());
    //     assert!(state.get_document(&new_uri).is_some());
    //     assert_eq!(
    //         state.get_content(&new_uri),
    //         "syntax = \"proto3\";\npackage com.test;\nmessage Book { string title = 1; }\nenum Color { RED = 0; }\n"
    //     );
    // }

    #[test]
    fn test_upsert_content_tracks_unresolved_imports() {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];
        let unresolved = state.upsert_content(
            &uri("file:///importing.proto"),
            "syntax = \"proto3\";\nimport \"nonexistent.proto\";\npackage com.test;\n",
            ipath,
            1,
        );
        assert_eq!(unresolved, vec!["nonexistent.proto"]);
    }

    #[test]
    fn test_upsert_content_resolved_imports() {
        let mut state = ProtoLanguageState::new();
        let dir = tempfile::tempdir().unwrap();
        let dep_path = dir.path().join("dep.proto");
        std::fs::write(&dep_path, "syntax = \"proto3\";\npackage com.dep;\n").unwrap();
        let ipath = vec![dir.path().to_path_buf()];

        let unresolved = state.upsert_content(
            &uri("file:///main.proto"),
            "syntax = \"proto3\";\nimport \"dep.proto\";\npackage com.main;\n",
            &ipath,
            1,
        );
        assert!(unresolved.is_empty());
        assert!(state.get_document(&uri("file:///main.proto")).is_some());
    }

    #[test]
    fn test_upsert_content_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        let a_path = dir.path().join("a.proto");
        let b_path = dir.path().join("b.proto");
        std::fs::write(
            &a_path,
            "syntax = \"proto3\";\nimport \"b.proto\";\npackage com.a;\n",
        )
        .unwrap();
        std::fs::write(
            &b_path,
            "syntax = \"proto3\";\nimport \"a.proto\";\npackage com.b;\n",
        )
        .unwrap();
        let ipath = vec![dir.path().to_path_buf()];

        // depth=0 should not parse anything
        let mut state0 = ProtoLanguageState::new();
        state0.upsert_content(
            &uri("file:///a.proto"),
            std::fs::read_to_string(&a_path).unwrap().as_str(),
            &ipath,
            0,
        );
        assert!(state0.get_document(&uri("file:///a.proto")).is_none());

        // depth=1 should parse a.proto but not follow imports
        let mut state1 = ProtoLanguageState::new();
        state1.upsert_content(
            &uri("file:///a.proto"),
            std::fs::read_to_string(&a_path).unwrap().as_str(),
            &ipath,
            1,
        );
        assert!(state1.get_document(&uri("file:///a.proto")).is_some());
        assert!(state1.get_document(&uri("file:///b.proto")).is_none());
    }

    #[test]
    fn test_parse_all_from_workspace() {
        let mut state = ProtoLanguageState::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.proto"),
            "syntax = \"proto3\";\npackage com.a;\nmessage A {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("b.proto"),
            "syntax = \"proto3\";\npackage com.b;\nmessage B {}\n",
        )
        .unwrap();
        // Non-proto file should be ignored
        std::fs::write(dir.path().join("notes.txt"), "hello").unwrap();

        state.parse_all_from_workspace(dir.path(), None);
        assert_eq!(state.get_documents().len(), 2);

        // Second call should be idempotent
        state.parse_all_from_workspace(dir.path(), None);
        assert_eq!(state.get_documents().len(), 2);
    }

    #[test]
    fn test_upsert_file_returns_diagnostics() {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];
        let result = state.upsert_file(
            &uri("file:///test.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book {}\n",
            ipath,
            1,
            &Config::default(),
            false,
        );
        assert!(result.is_some());
        let params = result.unwrap();
        assert_eq!(params.uri.as_str(), "file:///test.proto");
        // Should have no diagnostics for valid proto
        assert!(params.diagnostics.is_empty());
    }

    #[test]
    fn test_upsert_file_returns_parse_diagnostics() {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];
        let result = state.upsert_file(
            &uri("file:///bad.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book { invalid syntax here }\n",
            ipath,
            1,
            &Config::default(),
            false,
        );
        assert!(result.is_some());
        let params = result.unwrap();
        assert!(
            !params.diagnostics.is_empty(),
            "expected parse diagnostics for invalid proto"
        );
    }
}
