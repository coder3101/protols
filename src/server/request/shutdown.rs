use async_lsp::ResponseError;
use futures::FutureExt;
use futures::future::BoxFuture;

use crate::ProtoLanguageServer;

impl ProtoLanguageServer {
    pub(in crate::server) fn shutdown(
        &mut self,
        _params: (),
    ) -> BoxFuture<'static, Result<(), ResponseError>> {
        tracing::info!("Received shutdown request");

        self.shutdown_token.cancel();

        async { Ok(()) }.boxed()
    }
}
