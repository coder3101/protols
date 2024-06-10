use async_lsp::{router::Router, ClientSocket};
use std::ops::ControlFlow;
use tracing::info;

pub struct TickEvent;
pub struct ServerState {
    pub client: ClientSocket,
    pub counter: i32,
}

impl ServerState {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let mut router = Router::from_language_server(Self { client, counter: 0 });
        router.event(Self::on_tick);
        router
    }

    fn on_tick(&mut self, _: TickEvent) -> ControlFlow<async_lsp::Result<()>> {
        info!("tick");
        self.counter += 1;
        ControlFlow::Continue(())
    }
}
