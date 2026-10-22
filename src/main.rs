use async_lsp::LanguageClient;
use async_lsp::client_monitor::ClientProcessMonitorLayer;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use clap::Parser;
use cli::Cli;
use server::ProtoLanguageServer;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;

use crate::transport::create_transport;

mod cli;
mod config;
mod docs;
mod document;
mod formatter;
mod log;
// mod lsp;
mod model;
mod protoc;
mod server;
// mod state;
mod transport;
mod utils;

const FALLBACK_INCLUDE_PATH: Option<&str> = option_env!("FALLBACK_INCLUDE_PATH");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), transport::TransportError> {
    let cli = Cli::parse();

    let shutdown_token = CancellationToken::new();

    tokio::spawn({
        let token = shutdown_token.clone();
        async move {
            utils::wait_for_shutdown_signals().await;
            token.cancel();
        }
    });

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let (reload_handle, _log_guard) = log::install(tx);

    tracing::info!("server version: {}", env!("CARGO_PKG_VERSION"));
    tracing::info!("CLI include paths: {:?}", &cli.include_paths);

    let shutdown_token_for_server = shutdown_token.child_token();

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
            shutdown_token_for_server,
        );

        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .layer(ClientProcessMonitorLayer::new(client))
            .service(router)
    });

    let (input, output) = create_transport(&cli).await?;

    if shutdown_token
        .run_until_cancelled(server.run_buffered(input, output))
        .await
        .is_none()
    {
        tracing::info!("Graceful shutdown complete. Exiting...");
    }

    Ok(())
}
