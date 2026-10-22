use std::fmt;
use std::ops::ControlFlow;

use async_lsp::lsp_types::notification::{self, Notification as N};
use async_lsp::{Result, lsp_types};
use tokio::sync::mpsc::error::TrySendError;

use super::ProtoLanguageServer;

use worker::WORKER_MESSAGE_BUFFER;

pub(super) mod worker;

#[derive(Debug)]
pub(super) enum Notification {
    Initialized {
        params: lsp_types::InitializedParams,
    },
    OpenDocument {
        params: lsp_types::DidOpenTextDocumentParams,
    },
    ChangeDocument {
        params: lsp_types::DidChangeTextDocumentParams,
    },
    SaveDocument {
        params: lsp_types::DidSaveTextDocumentParams,
    },
    CloseDocument {
        params: lsp_types::DidCloseTextDocumentParams,
    },
    CreateFiles {
        params: lsp_types::CreateFilesParams,
    },
    RenameFiles {
        params: lsp_types::RenameFilesParams,
    },
    DeleteFiles {
        params: lsp_types::DeleteFilesParams,
    },
    ChangeWorkspaceFolders {
        params: lsp_types::DidChangeWorkspaceFoldersParams,
    },
    ChangeWatchedFiles {
        params: lsp_types::DidChangeWatchedFilesParams,
    },
    SetTrace {
        params: lsp_types::SetTraceParams,
    },
    Exit,
}

impl ProtoLanguageServer {
    pub(super) fn handle_notification<N>(&mut self, params: N::Params) -> ControlFlow<Result<()>>
    where
        N: lsp_types::notification::Notification,
        N::Params: Into<Notification>,
    {
        match self.notification_tx.try_send(params.into()) {
            Err(TrySendError::Full(_)) => {
                tracing::error!(
                    method = N::METHOD,
                    WORKER_MESSAGE_BUFFER,
                    "CRITICAL: Notification worker channel is FULL"
                );

                return ControlFlow::Break(Err(async_lsp::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    format!("Notification channel overflowed on method: {}", N::METHOD),
                ))));
            }
            Err(TrySendError::Closed(_)) => {
                tracing::error!(
                    method = N::METHOD,
                    "CRITICAL: Notification worker actor crashed or dropped its receiver!"
                );

                return ControlFlow::Break(Err(async_lsp::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "Notification worker crashed unexpectedly",
                ))));
            }
            Ok(()) => {}
        }

        if N::METHOD != lsp_types::notification::Exit::METHOD {
            return ControlFlow::Continue(());
        }

        if self.shutdown_token.is_cancelled() {
            return ControlFlow::Break(Ok(()));
        }

        ControlFlow::Break(Err(async_lsp::Error::Protocol(
            "Received 'exit' notification without a prior 'shutdown' request".to_string(),
        )))
    }
}

impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::ChangeDocument { .. } => notification::DidChangeTextDocument::METHOD,
            Self::ChangeWatchedFiles { .. } => notification::DidChangeWatchedFiles::METHOD,
            Self::ChangeWorkspaceFolders { .. } => notification::DidChangeWorkspaceFolders::METHOD,
            Self::CloseDocument { .. } => notification::DidCloseTextDocument::METHOD,
            Self::CreateFiles { .. } => notification::DidCreateFiles::METHOD,
            Self::DeleteFiles { .. } => notification::DidDeleteFiles::METHOD,
            Self::Exit => notification::Exit::METHOD,
            Self::Initialized { .. } => notification::Initialized::METHOD,
            Self::OpenDocument { .. } => notification::DidOpenTextDocument::METHOD,
            Self::RenameFiles { .. } => notification::DidRenameFiles::METHOD,
            Self::SaveDocument { .. } => notification::DidSaveTextDocument::METHOD,
            Self::SetTrace { .. } => notification::SetTrace::METHOD,
        };
        f.write_str(s)
    }
}
