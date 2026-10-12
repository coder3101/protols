use std::collections::HashMap;
use std::sync::Arc;

use async_lsp::lsp_types::Url;
use futures::future::Either;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::{FutureExt, Stream};
use tokio_util::sync::CancellationToken;
use tree_sitter::LanguageError;

use super::cache::get_uri;
use super::cache::{ParsingResult, SharedParsingFuture};
use super::error::CoreError;
use super::finalizer::FinalizerToken;
use super::unique_uris::UniqueUris;
use super::{CacheEntry, DocumentVersion, ProtoLanguageState};
use crate::document::ProtoDocument;
use crate::server::progress::{LspProgressReporter, OptionReporterExt};

const WRITE_CHUNK_SIZE: usize = 16;
const MAX_PROTO_FILE_SIZE_BYTES: u64 = 1024 * 1024;

enum TaskSource {
    FromMemory(Arc<String>),
    FromDisk,
}

type ParsingTask = (
    Url,
    TaskSource,
    tokio::sync::oneshot::Sender<Result<Arc<ProtoDocument>, CoreError>>,
    CancellationToken,
);

impl ProtoLanguageState {
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
    ) -> (FinalizerToken, impl Stream<Item = ParsingResult>) {
        {
            let documents = self.documents.read().await;
            if let Some((cache_hits, client_futures)) =
                Self::probe_cache_fast_path(&uris, need_results, &documents)
            {
                return (
                    FinalizerToken::dummy(),
                    Either::Left(futures::stream::iter(cache_hits).chain(client_futures)),
                );
            }
        }

        let mut cache_hits = Vec::new();
        let mut client_futures = FuturesUnordered::new();
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
                    && matches!(entry, Some(CacheEntry::Dirty | CacheEntry::Ready { .. }))
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

                let (tx, rx) = tokio::sync::oneshot::channel();
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

    fn probe_cache_fast_path(
        uris: &UniqueUris,
        need_results: bool,
        documents: &HashMap<Url, CacheEntry>,
    ) -> Option<(Vec<ParsingResult>, FuturesUnordered<SharedParsingFuture>)> {
        let mut cache_hits = Vec::new();
        let mut client_futures = FuturesUnordered::new();

        for uri in uris {
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
                    return None;
                }
            }
        }

        Some((cache_hits, client_futures))
    }

    fn run_blocking_parser(&self, tasks: Vec<ParsingTask>) {
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
                    let Ok(path) = uri.to_file_path() else {
                        let _ = tx.send(Err(CoreError::InvalidPath { uri }));
                        continue;
                    };

                    if path.extension().is_none_or(|ext| ext != "proto") {
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
        Fut: Future<Output = (DocumentVersion, ParsingResult)> + Send + 'static,
    {
        if futures.is_empty() {
            tracing::warn!(
                "Document cache finalizer called with 0 tasks. This might indicate lost futures."
            );

            return FinalizerToken::dummy();
        }

        let total = u32::try_from(futures.len()).unwrap_or(u32::MAX);
        let mut completed = 0;
        let mut chunks = futures.ready_chunks(WRITE_CHUNK_SIZE);
        let documents_clone = self.documents.clone();

        let handle = tokio::spawn(async move {
            while let Some(chunk) = chunks.next().await {
                {
                    let mut documents = documents_clone.write().await;

                    for (expected_version, result) in chunk {
                        if let Some(document) = documents.get_mut(get_uri(&result)) {
                            document.finalize(expected_version, result);
                        }

                        completed += 1;
                    }
                }

                let percentage = completed * 100 / total;
                progress_reporter.report(percentage, &format!("Parsing [{completed}/{total}]"));

                tokio::task::yield_now().await;
            }
        });

        FinalizerToken::from_handle(handle)
    }

    pub async fn get_document(&self, uri: &Url, cancel_token: CancellationToken) -> ParsingResult {
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

    pub async fn get_document_text(&self, uri: &Url) -> Option<Arc<String>> {
        self.sources
            .read()
            .await
            .get(uri)
            .map(|se| se.content.clone())
    }
}
