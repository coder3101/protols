use async_lsp::{lsp_types::Url, router::Router, ClientSocket, ErrorCode, ResponseError};
use std::{collections::HashMap, ops::ControlFlow};
use tracing::error;

use crate::parser::{ParsedTree, ProtoParser};

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

    pub fn get_parsed_tree_and_content(
        &mut self,
        uri: &Url,
    ) -> Result<(ParsedTree, &str), ResponseError> {
        let Some(content) = self.documents.get(uri) else {
            error!("failed to get document at {uri}");
            return Err(ResponseError::new(
                ErrorCode::INVALID_REQUEST,
                "uri was never opened",
            ));
        };

        let Some(parsed) = self.parser.parse(content.as_bytes()) else {
            error!("failed to parse content at {uri}");
            return Err(ResponseError::new(
                ErrorCode::REQUEST_FAILED,
                "ts failed to parse contents",
            ));
        };

        Ok((parsed, content))
    }
}
