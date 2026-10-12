use async_lsp::ResponseError;
use futures::future::{Ready, ready};

use crate::ProtoLanguageServer;

impl ProtoLanguageServer {
    pub(in crate::server) fn shutdown(&mut self, _params: ()) -> Ready<Result<(), ResponseError>> {
        tracing::info!("Received shutdown request");

        self.shutdown_token.cancel();

        ready(Ok(()))
    }
}
