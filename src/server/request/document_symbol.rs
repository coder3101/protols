use async_lsp::ResponseError;
use async_lsp::lsp_types::{DocumentSymbolParams, DocumentSymbolResponse};
use futures::FutureExt;
use futures::future::BoxFuture;

use crate::ProtoLanguageServer;

impl ProtoLanguageServer {
    pub(in crate::server) fn document_symbol(
        &mut self,
        params: DocumentSymbolParams,
    ) -> BoxFuture<'static, Result<Option<DocumentSymbolResponse>, ResponseError>> {
        let state = self.state.clone();
        let uri = params.text_document.uri;
        let guard = self.new_request_guard();

        async move {
            let document = state.get_document(&uri, guard.token().clone()).await?;
            let symbols = document.document_symbols();
            let response = DocumentSymbolResponse::Nested(symbols);

            Ok(Some(response))
        }
        .boxed()
    }
}
