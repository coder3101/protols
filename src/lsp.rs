use std::ops::ControlFlow;
use tracing::{error, info};

use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InsertTextFormat, OneOf, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind,
};
use async_lsp::{LanguageClient, LanguageServer, ResponseError};
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
            .map(|c| (c.name.as_str(), c.version.as_deref()))
            .unwrap_or(("<unknown>", None));

        let cversion = version.unwrap_or("<unknown>");

        info!("Connected with client {cname} {cversion}");

        let response = InitializeResult {
            capabilities: ServerCapabilities {
                // todo(): We might prefer incremental sync at some later stage
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
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

        match self.get_parsed_tree_and_content(&uri) {
            Err(e) => Box::pin(async move { Err(e) }),
            Ok((tree, content)) => {
                let comments = tree.hover(&pos, content.as_bytes());

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
        }
    }
    fn completion(
        &mut self,
        _params: CompletionParams,
    ) -> BoxFuture<'static, Result<Option<CompletionResponse>, Self::Error>> {
        let keywords = vec![
            "syntax", "package", "option", "import", "service", "rpc", "returns", "message",
            "enum", "oneof", "repeated", "reserved", "to",
        ];

        let keywords = keywords
            .into_iter()
            .map(|w| CompletionItem {
                label: w.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..CompletionItem::default()
            })
            .collect();

        Box::pin(async move { Ok(Some(CompletionResponse::Array(keywords))) })
    }

    fn definition(
        &mut self,
        param: GotoDefinitionParams,
    ) -> BoxFuture<'static, Result<Option<GotoDefinitionResponse>, ResponseError>> {
        let uri = param.text_document_position_params.text_document.uri;
        let pos = param.text_document_position_params.position;

        match self.get_parsed_tree_and_content(&uri) {
            Err(e) => Box::pin(async move { Err(e) }),
            Ok((tree, content)) => {
                let locations = tree.definition(&pos, &uri, content.as_bytes());

                let response = match locations.len() {
                    0 => None,
                    1 => Some(GotoDefinitionResponse::Scalar(locations[0].clone())),
                    2.. => Some(GotoDefinitionResponse::Array(locations)),
                };

                Box::pin(async move { Ok(response) })
            }
        }
    }

    fn document_symbol(
        &mut self,
        params: DocumentSymbolParams,
    ) -> BoxFuture<'static, Result<Option<DocumentSymbolResponse>, Self::Error>> {
        let uri = params.text_document.uri;

        match self.get_parsed_tree_and_content(&uri) {
            Err(e) => Box::pin(async move { Err(e) }),
            Ok((tree, content)) => {
                let locations = tree.find_document_locations(content.as_bytes());
                let response = DocumentSymbolResponse::Nested(locations);

                Box::pin(async move { Ok(Some(response)) })
            }
        }
    }

    fn did_save(&mut self, _: DidSaveTextDocumentParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let contents = params.text_document.text;

        info!("opened file at: {uri}");
        self.documents.insert(uri.clone(), contents.clone());

        let Some(tree) = self.parser.parse(contents.as_bytes()) else {
            error!("failed to parse content at {uri}");
            return ControlFlow::Continue(());
        };

        let diagnostics = tree.collect_parse_errors(&uri);
        if let Err(e) = self.client.publish_diagnostics(diagnostics) {
            error!(error=%e, "failed to publish diagnostics")
        }
        ControlFlow::Continue(())
    }

    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;

        info!("closed file at {uri}");
        self.documents.remove(&uri);

        ControlFlow::Continue(())
    }

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let contents = params.content_changes[0].text.clone();

        self.documents.insert(uri.clone(), contents.clone());

        let Some(tree) = self.parser.parse(contents.as_bytes()) else {
            error!("failed to parse content at {uri}");
            return ControlFlow::Continue(());
        };

        let diagnostics = tree.collect_parse_errors(&uri);
        if let Err(e) = self.client.publish_diagnostics(diagnostics) {
            error!(error=%e, "failed to publish diagnostics")
        }
        ControlFlow::Continue(())
    }
}
