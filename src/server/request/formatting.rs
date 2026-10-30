use std::sync::Arc;

use async_lsp::lsp_types::{
    DocumentFormattingParams, DocumentRangeFormattingParams, TextEdit, Url,
};
use async_lsp::{ErrorCode, ResponseError};
use futures::FutureExt;
use futures::future::BoxFuture;

use crate::ProtoLanguageServer;
use crate::formatter::clang::ClangFormatter;

impl ProtoLanguageServer {
    pub(in crate::server) fn formatting(
        &mut self,
        params: DocumentFormattingParams,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, ResponseError>> {
        let uri = params.text_document.uri;

        self.execute_formatting_pipeline(uri.clone(), move |formatter, text| async move {
            formatter.format_document(uri.path(), text.as_str()).await
        })
    }

    pub(in crate::server) fn range_formatting(
        &mut self,
        params: DocumentRangeFormattingParams,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, ResponseError>> {
        let range = params.range;
        let uri = params.text_document.uri;

        self.execute_formatting_pipeline(uri.clone(), move |formatter, text| async move {
            formatter
                .format_document_range(&range, uri.path(), text.as_str())
                .await
        })
    }

    fn execute_formatting_pipeline<F, Fut>(
        &self,
        uri: Url,
        f: F,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, ResponseError>>
    where
        F: FnOnce(Arc<ClangFormatter>, Arc<String>) -> Fut + Send + 'static,
        Fut: futures::Future<Output = Option<Vec<TextEdit>>> + Send + 'static,
    {
        let state = self.state.clone();
        let config = self.configs.clone();
        let guard = self.new_request_guard();

        let uri_clone = uri.clone();
        let formatter = async move {
            config
                .read()
                .await
                .get_formatter_for_uri(&uri_clone)
                .ok_or_else(|| ResponseError::new(ErrorCode::INTERNAL_ERROR, "Formatter not found"))
        };

        let text = async move {
            state.get_document_text(&uri).await.ok_or_else(|| {
                ResponseError::new(ErrorCode::INTERNAL_ERROR, "Document text not found")
            })
        };

        let formatting_pipeline =
            futures::future::try_join(formatter, text).then(move |res| async move {
                let (formatter, text) = res?;
                Ok(f(formatter, text).await)
            });

        async move {
            guard
                .token()
                .run_until_cancelled(formatting_pipeline)
                .await
                .unwrap_or_else(|| {
                    Err(ResponseError::new(
                        ErrorCode::REQUEST_CANCELLED,
                        "Formatting request was cancelled",
                    ))
                })
        }
        .boxed()
    }
}
