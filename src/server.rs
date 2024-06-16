use async_lsp::{lsp_types::Url, router::Router, ClientSocket};
use std::{collections::HashMap, ops::ControlFlow};

use crate::parser::ProtoParser;

pub struct TickEvent;
pub struct ServerState {
    pub client: ClientSocket,
    pub counter: i32,
    pub documents: HashMap<Url, String>,
    pub parser: ProtoParser,
}

impl ServerState {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let mut router = Router::from_language_server(Self {
            client,
            counter: 0,
            documents: Default::default(),
            parser: ProtoParser::new(),
        });
        router.event(Self::on_tick);
        router
    }

    fn on_tick(&mut self, _: TickEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.counter += 1;
        ControlFlow::Continue(())
    }
}
