use std::time::Duration;

use async_lsp::client_monitor::ClientProcessMonitorLayer;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use server::{ProtoLanguageServer, TickEvent};
use tower::ServiceBuilder;
use tracing::Level;

mod config;
mod context;
mod formatter;
mod lsp;
mod nodekind;
mod parser;
mod server;
mod state;
mod utils;
mod workspace;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--version" {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }

    let (server, _) = async_lsp::MainLoop::new_server(|client| {
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
            .service(ProtoLanguageServer::new_router(client))
    });

    let dir = std::env::temp_dir();
    eprintln!("Logs are being written to directory {:?}", dir);

    let file_appender = tracing_appender::rolling::daily(dir, "protols.log");
    let (non_blocking, _gaurd) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .with_writer(non_blocking)
        .init();

    // Prefer truly asynchronous piped stdin/stdout without blocking tasks.
    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().unwrap(),
        async_lsp::stdio::PipeStdout::lock_tokio().unwrap(),
    );
    // Fallback to spawn blocking read/write otherwise.
    #[cfg(not(unix))]
    let (stdin, stdout) = (
        tokio_util::compat::TokioAsyncReadCompatExt::compat(tokio::io::stdin()),
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tokio::io::stdout()),
    );

    server.run_buffered(stdin, stdout).await.unwrap();
}
