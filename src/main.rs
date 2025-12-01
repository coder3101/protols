use std::time::Duration;

use async_lsp::client_monitor::ClientProcessMonitorLayer;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use clap::Parser;
use server::{ProtoLanguageServer, TickEvent};
use tower::ServiceBuilder;
use tracing::Level;

mod config;
mod context;
mod docs;
mod formatter;
mod lsp;
mod nodekind;
mod parser;
mod protoc;
mod server;
mod state;
mod utils;
mod workspace;

/// Language server for proto3 files
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, ignore_errors(true))]
struct Cli {
    /// Include paths for proto files
    #[arg(short, long, value_delimiter = ',')]
    include_paths: Option<Vec<String>>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    let dir = std::env::temp_dir();
    eprintln!("file logging at directory: {dir:?}");

    let file_appender = tracing_appender::rolling::daily(dir.clone(), "protols.log");
    let file_appender = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .with_writer(file_appender.0)
        .init();

    let fallback_include_path = option_env!("FALLBACK_INCLUDE_PATH").map(Into::into);

    tracing::info!("server version: {}", env!("CARGO_PKG_VERSION"));
    let (server, _) = async_lsp::MainLoop::new_server(|client| {
        tracing::info!("Using CLI options: {:?}", cli);
        tracing::info!("Using fallback include path: {:?}", fallback_include_path);
        let router = ProtoLanguageServer::new_router(
            client.clone(),
            cli.include_paths
                .map(|ic| ic.into_iter().map(std::path::PathBuf::from).collect())
                .unwrap_or_default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test with no arguments
        let args = vec!["protols"];
        let cli = Cli::parse_from(args);
        assert!(cli.include_paths.is_none());

        // Test with include paths
        let args = vec!["protols", "--include-paths=/path1,/path2"];
        let cli = Cli::parse_from(args);
        assert!(cli.include_paths.is_some());
        let paths = cli.include_paths.unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "/path1");
        assert_eq!(paths[1], "/path2");

        // Test with short form
        let args = vec!["protols", "-i", "/path1,/path2"];
        let cli = Cli::parse_from(args);
        assert!(cli.include_paths.is_some());
        let paths = cli.include_paths.unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "/path1");
        assert_eq!(paths[1], "/path2");
    }
}
