use async_lsp::lsp_types::Url;
use async_lsp::{ErrorCode, ResponseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoreError {
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

impl From<CoreError> for ResponseError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::DocumentNotFound { uri } => ResponseError::new(
                ErrorCode::INVALID_PARAMS,
                format!("Protols cache failure: Document metadata wrapper not found {uri}"),
            ),

            CoreError::SourceNotFound { uri } => ResponseError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Protols cache corruption: Raw source text missing for tracked file {uri}"),
            ),

            CoreError::InvalidPath { uri } => ResponseError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Protols failed to convert URI to OS path: {uri}"),
            ),

            CoreError::InvalidExtension { uri } => ResponseError::new(
                ErrorCode::REQUEST_FAILED,
                format!("Unsupported file extension. Protols only handles '.proto' files: {uri}"),
            ),

            CoreError::FileTooLarge { uri, size } => ResponseError::new(
                ErrorCode::REQUEST_FAILED,
                format!("File exceeds maximum allowed size for parsing ({size} bytes): {uri}"),
            ),

            CoreError::DiskIoError { details, uri } => ResponseError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Protols failed to read from disk: {uri}. Details: {details}"),
            ),

            CoreError::ParserError { uri } => ResponseError::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Protols failed to create parser for URI {uri}"),
            ),

            CoreError::RequestCancelled { uri } => ResponseError::new(
                ErrorCode::REQUEST_CANCELLED,
                format!("LSP request was aborted or superseded by a newer action for URI {uri}"),
            ),

            CoreError::InternalError { uri } => ResponseError::new(
                ErrorCode::INTERNAL_ERROR,
                format!(
                    "Protols internal panic: Batch processing engine lost requested file map entry {uri}"
                ),
            ),
        }
    }
}
