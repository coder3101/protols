use std::collections::HashMap;
use std::sync::Arc;

use async_lsp::lsp_types::Url;
use futures::future::{BoxFuture, Shared};
use tokio::sync::RwLockWriteGuard;
use tokio_util::sync::CancellationToken;

use super::DocumentVersion;
use super::error::CoreError;
use crate::document::ProtoDocument;

pub(super) type ParsingResult = Result<Arc<ProtoDocument>, CoreError>;

pub(super) type SharedParsingFuture = Shared<BoxFuture<'static, ParsingResult>>;

#[derive(Clone)]
pub enum CacheEntry {
    Ready {
        version: DocumentVersion,
        result: ParsingResult,
    },
    Pending {
        version: DocumentVersion,
        future: SharedParsingFuture,
        cancel_token: CancellationToken,
    },
    Dirty,
}

impl CacheEntry {
    pub(super) fn is_transient_error(result: &Result<Arc<ProtoDocument>, CoreError>) -> bool {
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

    pub(super) fn finalize(
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

pub(super) fn get_uri(result: &Result<Arc<ProtoDocument>, CoreError>) -> &Url {
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
