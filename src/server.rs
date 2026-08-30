use async_lsp::{
    ClientSocket,
    lsp_types::{
        notification::{
            DidChangeTextDocument, DidCreateFiles, DidDeleteFiles, DidOpenTextDocument,
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
use std::{ops::ControlFlow, path::PathBuf};
use tokio_util::sync::CancellationToken;

use crate::{config::WorkspaceProtoConfigs, log, state::ProtoLanguageState};

mod lifecycle;

pub struct TickEvent;
pub struct ProtoLanguageServer {
    pub client: ClientSocket,
    pub(crate) log_handle: log::LogReloadHandle,
    pub counter: i32,
    pub state: ProtoLanguageState,
    pub configs: WorkspaceProtoConfigs,
    pub shutdown_received: bool,
    pub shutdown_cancel_token: CancellationToken,
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
            shutdown_cancel_token: CancellationToken::new(),
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
        router.request::<Initialize, _>(Self::initialize);
        router.request::<Shutdown, _>(Self::shutdown);
        router.request::<HoverRequest, _>(Self::hover);
        router.request::<Completion, _>(Self::completion);
        router.request::<PrepareRenameRequest, _>(Self::prepare_rename);
        router.request::<Rename, _>(Self::rename);
        router.request::<References, _>(Self::references);
        router.request::<GotoDefinition, _>(Self::definition);
        router.request::<DocumentSymbolRequest, _>(Self::document_symbol);
        router.request::<WorkspaceSymbolRequest, _>(Self::workspace_symbol);
        router.request::<Formatting, _>(Self::formatting);
        router.request::<RangeFormatting, _>(Self::range_formatting);

        // Handling notification
        router.notification::<Initialized>(Self::initialized);
        router.notification::<SetTrace>(Self::set_trace);
        router.notification::<DidSaveTextDocument>(Self::did_save);
        router.notification::<DidOpenTextDocument>(Self::did_open);
        router.notification::<DidChangeTextDocument>(Self::did_change);
        router.notification::<DidCreateFiles>(Self::did_create_files);
        router.notification::<DidRenameFiles>(Self::did_rename_files);
        router.notification::<DidDeleteFiles>(Self::did_delete_files);
        router.notification::<Exit>(Self::exit);

        router
    }
}
