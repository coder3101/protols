use async_lsp::{
    ClientSocket, LanguageClient,
    lsp_types::{
        NumberOrString, ProgressParams, ProgressParamsValue,
        notification::{
            DidChangeTextDocument, DidCreateFiles, DidDeleteFiles, DidOpenTextDocument,
            DidRenameFiles, DidSaveTextDocument,
        },
        request::{
            Completion, DocumentSymbolRequest, Formatting, GotoDefinition, HoverRequest,
            Initialize, PrepareRenameRequest, RangeFormatting, References, Rename,
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

use crate::{config::workspace::WorkspaceProtoConfigs, state::ProtoLanguageState};

pub struct TickEvent;
pub struct ProtoLanguageServer {
    pub client: ClientSocket,
    pub counter: i32,
    pub state: ProtoLanguageState,
    pub configs: WorkspaceProtoConfigs,
}

impl ProtoLanguageServer {
    pub fn new_router(
        client: ClientSocket,
        cli_include_paths: Vec<PathBuf>,
        fallback_include_path: Option<PathBuf>,
    ) -> Router<Self> {
        let mut router = Router::new(Self {
            client,
            counter: 0,
            state: ProtoLanguageState::new(),
            configs: WorkspaceProtoConfigs::new(cli_include_paths, fallback_include_path),
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
        router.request::<Initialize, _>(|st, params| st.initialize(params));
        router.request::<HoverRequest, _>(|st, params| st.hover(params));
        router.request::<Completion, _>(|st, params| st.completion(params));
        router.request::<PrepareRenameRequest, _>(|st, params| st.prepare_rename(params));
        router.request::<Rename, _>(|st, params| st.rename(params));
        router.request::<References, _>(|st, params| st.references(params));
        router.request::<GotoDefinition, _>(|st, params| st.definition(params));
        router.request::<DocumentSymbolRequest, _>(|st, params| st.document_symbol(params));
        router.request::<WorkspaceSymbolRequest, _>(|st, params| st.workspace_symbol(params));
        router.request::<Formatting, _>(|st, params| st.formatting(params));
        router.request::<RangeFormatting, _>(|st, params| st.range_formatting(params));

        // Handling notification
        router.notification::<DidSaveTextDocument>(|st, params| st.did_save(params));
        router.notification::<DidOpenTextDocument>(|st, params| st.did_open(params));
        router.notification::<DidChangeTextDocument>(|st, params| st.did_change(params));
        router.notification::<DidCreateFiles>(|st, params| st.did_create_files(params));
        router.notification::<DidRenameFiles>(|st, params| st.did_rename_files(params));
        router.notification::<DidDeleteFiles>(|st, params| st.did_delete_files(params));

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
