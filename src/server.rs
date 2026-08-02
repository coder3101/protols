use async_lsp::{
    ClientSocket, LanguageClient,
    lsp_types::{
        NumberOrString, ProgressParams, ProgressParamsValue,
        notification::{
            DidChangeTextDocument, DidCreateFiles, DidDeleteFiles, DidOpenTextDocument,
            DidRenameFiles, DidSaveTextDocument, Exit, SetTrace,
        },
        request::{
            Completion, DocumentSymbolRequest, Formatting, GotoDefinition, HoverRequest,
            Initialize, PrepareRenameRequest, RangeFormatting, References, Rename, Shutdown,
            WorkspaceSymbolRequest,
        },
    },
    router::Router,
};
use std::{
    ops::ControlFlow,
    path::PathBuf,
    sync::{mpsc, mpsc::Sender},
    thread,
};

use crate::{config::workspace::WorkspaceProtoConfigs, log, state::ProtoLanguageState};

pub struct TickEvent;
pub struct ProtoLanguageServer {
    pub client: ClientSocket,
    pub(crate) log_handle: log::LogReloadHandle,
    pub counter: i32,
    pub state: ProtoLanguageState,
    pub configs: WorkspaceProtoConfigs,
    pub shutdown_received: bool,
}

impl ProtoLanguageServer {
    pub fn new_router(
        client: ClientSocket,
        log_handle: log::LogReloadHandle,
        cli_include_paths: Vec<PathBuf>,
        fallback_include_path: Option<PathBuf>,
    ) -> Router<Self> {
        let mut router = Router::new(Self {
            client,
            log_handle,
            counter: 0,
            state: ProtoLanguageState::new(),
            configs: WorkspaceProtoConfigs::new(cli_include_paths, fallback_include_path),
            shutdown_received: false,
        });

        router.event::<TickEvent>(|st, _| {
            st.counter += 1;
            ControlFlow::Continue(())
        });

        // Ignore any unknown notification.
        router.unhandled_notification(|_, notif| {
            tracing::info!(notif.method, "ignored unknown notification");
            ControlFlow::Continue(())
        });

        // Handling request
        router.request::<Initialize, _>(ProtoLanguageServer::initialize);
        router.request::<Shutdown, _>(ProtoLanguageServer::shutdown);
        router.request::<HoverRequest, _>(|st, params| st.hover(params));
        router.request::<Completion, _>(ProtoLanguageServer::completion);
        router.request::<PrepareRenameRequest, _>(ProtoLanguageServer::prepare_rename);
        router.request::<Rename, _>(ProtoLanguageServer::rename);
        router.request::<References, _>(ProtoLanguageServer::references);
        router.request::<GotoDefinition, _>(ProtoLanguageServer::definition);
        router.request::<DocumentSymbolRequest, _>(|st, params| st.document_symbol(params));
        router.request::<WorkspaceSymbolRequest, _>(ProtoLanguageServer::workspace_symbol);
        router.request::<Formatting, _>(ProtoLanguageServer::formatting);
        router.request::<RangeFormatting, _>(ProtoLanguageServer::range_formatting);

        // Handling notification
        router.notification::<SetTrace>(ProtoLanguageServer::set_trace);
        router.notification::<DidSaveTextDocument>(ProtoLanguageServer::did_save);
        router.notification::<DidOpenTextDocument>(ProtoLanguageServer::did_open);
        router.notification::<DidChangeTextDocument>(ProtoLanguageServer::did_change);
        router.notification::<DidCreateFiles>(ProtoLanguageServer::did_create_files);
        router.notification::<DidRenameFiles>(ProtoLanguageServer::did_rename_files);
        router.notification::<DidDeleteFiles>(ProtoLanguageServer::did_delete_files);
        router.notification::<Exit>(ProtoLanguageServer::exit);

        router
    }

    pub fn with_report_progress(&self, token: NumberOrString) -> Sender<ProgressParamsValue> {
        let (tx, rx) = mpsc::channel();
        let mut socket = self.client.clone();

        thread::spawn(move || {
            while let Ok(value) = rx.recv() {
                if let Err(e) = socket.progress(ProgressParams {
                    token: token.clone(),
                    value,
                }) {
                    tracing::error!(error=%e, "failed to report parse progress");
                }
            }
        });

        tx
    }
}
