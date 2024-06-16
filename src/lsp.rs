use std::ops::ControlFlow;
use std::time::Duration;
use tracing::{debug, info};

use async_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    HoverProviderCapability, InitializeParams, InitializeResult, MarkedString, OneOf,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use async_lsp::{ErrorCode, LanguageServer, ResponseError};
use futures::future::BoxFuture;

use crate::server::ServerState;

impl LanguageServer for ServerState {
    type Error = ResponseError;
    type NotifyResult = ControlFlow<async_lsp::Result<()>>;

    fn initialize(
        &mut self,
        params: InitializeParams,
    ) -> BoxFuture<'static, Result<InitializeResult, Self::Error>> {
        let (cname, version) = params
            .client_info
            .as_ref()
            .map(|c| (c.name.as_str(), c.version.as_ref().map(|x| x.as_str())))
            .unwrap_or(("<unknown>", None));

        let cversion = version.unwrap_or("<unknown>");

        info!("Connected with client {cname} {cversion}");
        debug!("Initialize with {params:?}");

        let response = InitializeResult {
            capabilities: ServerCapabilities {
                // todo(): We might prefer incremental sync at some later stage
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        };

        Box::pin(async move { Ok(response) })
    }

    fn hover(
        &mut self,
        param: HoverParams,
    ) -> BoxFuture<'static, Result<Option<Hover>, Self::Error>> {
        let uri = param.text_document_position_params.text_document.uri;
        let pos = param.text_document_position_params.position;

        let Some(contents) = self.documents.get(&uri) else {
            return Box::pin(async move {
                Err(ResponseError::new(
                    ErrorCode::INVALID_REQUEST,
                    "uri was never opened",
                ))
            });
        };

        let Some(parsed) = self.parser.parse(contents.as_bytes()) else {
            return Box::pin(async move {
                Err(ResponseError::new(
                    ErrorCode::REQUEST_FAILED,
                    "ts failed to parse contents",
                ))
            });
        };

        let comments = parsed.hover(&pos, contents.as_bytes());
        info!("Found {} node comments in the document", comments.len());
        let response = match comments.len() {
            0 => None,
            1 => Some(Hover {
                contents: HoverContents::Scalar(comments[0].clone()),
                range: None,
            }),
            2.. => Some(Hover {
                contents: HoverContents::Array(comments),
                range: None,
            }),
        };

        Box::pin(async move { Ok(response) })
    }

    fn definition(
        &mut self,
        param: GotoDefinitionParams,
    ) -> BoxFuture<'static, Result<Option<GotoDefinitionResponse>, ResponseError>> {
        let uri = param.text_document_position_params.text_document.uri;
        let pos = param.text_document_position_params.position;

        let Some(contents) = self.documents.get(&uri) else {
            return Box::pin(async move {
                Err(ResponseError::new(
                    ErrorCode::INVALID_REQUEST,
                    "uri was never opened",
                ))
            });
        };

        let Some(parsed) = self.parser.parse(contents.as_bytes()) else {
            return Box::pin(async move {
                Err(ResponseError::new(
                    ErrorCode::REQUEST_FAILED,
                    "ts failed to parse contents",
                ))
            });
        };

        let locations = parsed.definition(&pos, &uri, contents.as_bytes());
        info!("Found {} matching nodes in the document", locations.len());

        let response = match locations.len() {
            0 => None,
            1 => Some(GotoDefinitionResponse::Scalar(locations[0].clone())),
            2.. => Some(GotoDefinitionResponse::Array(locations)),
        };

        Box::pin(async move { Ok(response) })
    }

    fn did_save(&mut self, _: DidSaveTextDocumentParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let contents = params.text_document.text;
        info!("Opened file at: {:}", uri);
        self.documents.insert(uri, contents);
        ControlFlow::Continue(())
    }

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let contents = params.content_changes[0].text.clone();
        self.documents.insert(uri, contents);
        ControlFlow::Continue(())
    }
}
