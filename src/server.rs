use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;

use async_lsp::{
    ClientSocket,
    lsp_types::{
        notification::{
            DidChangeTextDocument, DidChangeWatchedFiles, DidChangeWorkspaceFolders,
            DidCloseTextDocument, DidCreateFiles, DidDeleteFiles, DidOpenTextDocument,
            DidRenameFiles, DidSaveTextDocument, Exit, Initialized, SetTrace,
        },
        request::{
            Completion, DocumentSymbolRequest, Formatting, GotoDefinition, HoverRequest,
            Initialize, PrepareRenameRequest, RangeFormatting, References, Rename, Shutdown,
            WorkspaceSymbolRequest,
        },
    },
    router::Router,
};
use tokio::sync::{RwLock, mpsc::Sender};
use tokio_util::sync::{CancellationToken, DropGuard};

use crate::{config::WorkspaceProtoConfigs, log};

use notification::Notification;
use notification::worker::Worker;
use state::ProtoLanguageState;

mod notification;
pub(crate) mod progress;
mod request;
pub(super) mod state;

pub struct ProtoLanguageServer {
    pub client: ClientSocket,
    pub state: ProtoLanguageState,
    pub configs: Arc<RwLock<WorkspaceProtoConfigs>>,
    pub shutdown_token: CancellationToken,
    notification_tx: Sender<Notification>,
}

impl ProtoLanguageServer {
    pub fn new(
        client: ClientSocket,
        log_handle: log::LogReloadHandle,
        cli_include_paths: Vec<PathBuf>,
        fallback_include_path: Option<PathBuf>,
        shutdown_token: CancellationToken,
    ) -> Self {
        let configs = Arc::new(RwLock::new(WorkspaceProtoConfigs::new(
            cli_include_paths,
            fallback_include_path,
        )));
        let state = ProtoLanguageState::new();
        let notification_tx = Worker::start(
            state.clone(),
            configs.clone(),
            log_handle,
            client.clone(),
            shutdown_token.child_token(),
        );

        Self {
            client,
            state,
            configs,
            shutdown_token,
            notification_tx,
        }
    }

    pub fn new_router(
        client: ClientSocket,
        log_handle: log::LogReloadHandle,
        cli_include_paths: Vec<PathBuf>,
        fallback_include_path: Option<PathBuf>,
        shutdown_token: CancellationToken,
    ) -> Router<Self> {
        let mut router = Router::new(Self::new(
            client,
            log_handle,
            cli_include_paths,
            fallback_include_path,
            shutdown_token,
        ));

        // Ignore any unknown notification.
        router.unhandled_notification(|_, notif| {
            tracing::info!(notif.method, "ignored unknown notification");
            ControlFlow::Continue(())
        });

        // Handling request
        router
            .request::<Initialize, _>(Self::initialize)
            .request::<Shutdown, _>(Self::shutdown)
            // .request::<HoverRequest, _>(Self::hover)
            // .request::<Completion, _>(Self::completion)
            // .request::<PrepareRenameRequest, _>(Self::prepare_rename)
            // .request::<Rename, _>(Self::rename)
            // .request::<References, _>(Self::references)
            // .request::<GotoDefinition, _>(Self::definition)
            .request::<DocumentSymbolRequest, _>(Self::document_symbol)
            // .request::<WorkspaceSymbolRequest, _>(Self::workspace_symbol)
            .request::<Formatting, _>(Self::formatting)
            .request::<RangeFormatting, _>(Self::range_formatting);

        // Handling notification
        router
            .notification::<Initialized>(Self::handle_notification::<Initialized>)
            .notification::<SetTrace>(Self::handle_notification::<SetTrace>)
            .notification::<DidSaveTextDocument>(Self::handle_notification::<DidSaveTextDocument>)
            .notification::<DidOpenTextDocument>(Self::handle_notification::<DidOpenTextDocument>)
            .notification::<DidChangeTextDocument>(
                Self::handle_notification::<DidChangeTextDocument>,
            )
            .notification::<DidCloseTextDocument>(Self::handle_notification::<DidCloseTextDocument>)
            .notification::<DidCreateFiles>(Self::handle_notification::<DidCreateFiles>)
            .notification::<DidRenameFiles>(Self::handle_notification::<DidRenameFiles>)
            .notification::<DidDeleteFiles>(Self::handle_notification::<DidDeleteFiles>)
            .notification::<DidChangeWorkspaceFolders>(
                Self::handle_notification::<DidChangeWorkspaceFolders>,
            )
            .notification::<DidChangeWatchedFiles>(
                Self::handle_notification::<DidChangeWatchedFiles>,
            )
            .notification::<Exit>(Self::handle_notification::<Exit>);

        router
    }

    #[inline]
    pub fn new_request_guard(&self) -> DropGuard {
        self.shutdown_token.child_token().drop_guard()
    }
}
