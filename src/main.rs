use std::time::Duration;

use async_lsp::LanguageClient;
use async_lsp::client_monitor::ClientProcessMonitorLayer;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use clap::Parser;
use cli::Cli;
use server::{ProtoLanguageServer, TickEvent};
use tower::ServiceBuilder;

use crate::transport::create_transport;

mod cli;
mod config;
mod docs;
mod document;
mod formatter;
mod log;
mod lsp;
mod model;
mod protoc;
mod server;
mod state;
mod transport;
mod utils;
mod workspace_symbol;

const FALLBACK_INCLUDE_PATH: Option<&str> = option_env!("FALLBACK_INCLUDE_PATH");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), transport::TransportError> {
    let cli = Cli::parse();

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let (reload_handle, _log_guard) = log::install(tx);

    tracing::info!("server version: {}", env!("CARGO_PKG_VERSION"));
    tracing::info!("CLI include paths: {:?}", &cli.include_paths);

    let (server, _) = async_lsp::MainLoop::new_server(|client| {
        let mut log_client = client.clone();

        tokio::spawn(async move {
            while let Some(params) = rx.recv().await {
                let _ = log_client.log_message(params);
            }
        });

        let include_paths = cli.get_include_paths();

        let fallback_include_path = FALLBACK_INCLUDE_PATH.map(std::path::PathBuf::from);

        tracing::info!("Using fallback include path: {:?}", fallback_include_path);

        let router = ProtoLanguageServer::new_router(
            client.clone(),
            reload_handle,
            include_paths,
            fallback_include_path,
        );

        tokio::spawn({
            let client = client.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    if client.emit(TickEvent).is_err() {
                        break;
                    }
                }
            }
        });

        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .layer(ClientProcessMonitorLayer::new(client.clone()))
            .service(router)
    });

    let (input, output) = create_transport(&cli).await?;

    server.run_buffered(input, output).await?;

    Ok(())
}
