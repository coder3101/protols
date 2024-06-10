use async_lsp::LanguageClient;
use std::ops::ControlFlow;
use std::time::Duration;
use tracing::{debug, info};

use async_lsp::lsp_types::{
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, MarkedString,
    MessageType, OneOf, ServerCapabilities, ServerInfo, ShowMessageParams,
};
use async_lsp::{LanguageServer, ResponseError};
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

        Box::pin(async move {
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    ..ServerCapabilities::default()
                },
                server_info: Some(ServerInfo {
                    name: env!("CARGO_PKG_NAME").to_string(),
                    version: Some(env!("CARGO_PKG_VERSION").to_string()),
                }),
            })
        })
    }

    fn hover(&mut self, _: HoverParams) -> BoxFuture<'static, Result<Option<Hover>, Self::Error>> {
        let mut client = self.client.clone();
        let counter = self.counter;
        Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            client
                .show_message(ShowMessageParams {
                    typ: MessageType::INFO,
                    message: "Hello LSP".into(),
                })
                .unwrap();
            Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "I am a hover text {counter}!"
                ))),
                range: None,
            }))
        })
    }

    // fn definition(
    //     &mut self,
    //     _: GotoDefinitionParams,
    // ) -> BoxFuture<'static, Result<Option<GotoDefinitionResponse>, ResponseError>> {
    //     unimplemented!("Not yet implemented!");
    // }

    fn did_save(&mut self, _: DidSaveTextDocumentParams) -> Self::NotifyResult {
        todo!("to implement")
    }

    fn did_open(&mut self, _: DidOpenTextDocumentParams) -> Self::NotifyResult {
        todo!("to implement")
    }

    fn did_change(&mut self, _: DidChangeTextDocumentParams) -> Self::NotifyResult {
        todo!("to implement")
    }
}
