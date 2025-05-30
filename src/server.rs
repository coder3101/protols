use async_lsp::{
    ClientSocket, LanguageClient,
    lsp_types::{NumberOrString, ProgressParams, ProgressParamsValue},
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
    pub fn new_router(client: ClientSocket, cli_include_paths: Vec<PathBuf>) -> Router<Self> {
        let mut router = Router::from_language_server(Self {
            client,
            counter: 0,
            state: ProtoLanguageState::new(),
            configs: WorkspaceProtoConfigs::new(cli_include_paths),
        });
        router.event(Self::on_tick);
        router
    }

    fn on_tick(&mut self, _: TickEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.counter += 1;
        ControlFlow::Continue(())
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
