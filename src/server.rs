use async_lsp::{router::Router, ClientSocket};
use std::ops::ControlFlow;

use crate::{config::workspace::WorkspaceProtoConfigs, state::ProtoLanguageState};

pub struct TickEvent;
pub struct ProtoLanguageServer {
    pub client: ClientSocket,
    pub counter: i32,
    pub state: ProtoLanguageState,
    pub configs: WorkspaceProtoConfigs,
}

impl ProtoLanguageServer {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let mut router = Router::from_language_server(Self {
            client,
            counter: 0,
            state: ProtoLanguageState::new(),
            configs: WorkspaceProtoConfigs::new(),
        });
        router.event(Self::on_tick);
        router
    }

    fn on_tick(&mut self, _: TickEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.counter += 1;
        ControlFlow::Continue(())
    }
}
