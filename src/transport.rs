use crate::cli::Cli;
use futures::io::{AsyncRead, AsyncWrite};
use std::error::Error;
use std::pin::Pin;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub type TransportError = Box<dyn Error + Send + Sync>;
pub type TransportResult<T> = Result<T, TransportError>;

pub type LspReader = Pin<Box<dyn AsyncRead>>;
pub type LspWriter = Pin<Box<dyn AsyncWrite>>;

/// Establishes the communication channel for the LSP server based on CLI arguments.
///
/// This function acts as a factory that selects and initializes the appropriate
/// transport layer. It checks the following options in order of priority:
/// 1. **TCP Port**: Listens on `127.0.0.1` with a specific port.
/// 2. **TCP Socket**: Listens on a custom IP/Port string.
/// 3. **Pipe**: Creates a Unix Domain Socket (POSIX) or a Named Pipe (Windows).
/// 4. **Stdio**: Uses optimized standard input/output streams (Default).
///
/// # Errors
///
/// Returns a [TransportError] if:
/// * The specified TCP port or socket address is already in use.
/// * A Unix socket cannot be created (e.g., due to file permissions or path conflicts).
/// * Windows Named Pipe creation fails due to access rights or naming violations.
pub async fn create_transport(cli: &Cli) -> TransportResult<(LspReader, LspWriter)> {
    if let Some(port) = cli.port {
        let addr = format!("127.0.0.1:{}", port);
        return create_tcp_transport(&addr).await;
    }

    if let Some(addr) = &cli.socket {
        return create_tcp_transport(addr).await;
    }

    if let Some(path) = &cli.pipe {
        return create_pipe_transport(path).await;
    }

    create_stdio_transport().await
}

async fn create_tcp_transport(address: &str) -> TransportResult<(LspReader, LspWriter)> {
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .inspect_err(|e| eprintln!("Error: Could not bind to {}: {}", address, e))?;

    eprintln!(
        "LSP server listening on TCP: {}. Waiting for client...",
        address
    );

    let (stream, _) = listener
        .accept()
        .await
        .inspect_err(|e| eprintln!("Error: Failed to accept connection: {}", e))?;

    eprintln!("Client connected");
    tracing::info!("Using TCP: {}", address);

    let (reader, writer) = tokio::io::split(stream);

    Ok((Box::pin(reader.compat()), Box::pin(writer.compat_write())))
}

async fn create_pipe_transport(path: &str) -> TransportResult<(LspReader, LspWriter)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        if let Ok(metadata) = std::fs::metadata(path) {
            if !metadata.file_type().is_socket() {
                return Err(format!(
                    "Path '{}' exists and is not a socket. Refusing to overwrite.",
                    path
                )
                .into());
            }

            // In Unix, we must remove the existing socket file before binding.
            // See https://man7.org/linux/man-pages/man7/unix.7.html#NOTES
            let _ = std::fs::remove_file(path);
        }

        let listener = tokio::net::UnixListener::bind(path)
            .inspect_err(|e| eprintln!("Failed to bind Unix domain socket {}: {}", path, e))?;

        eprintln!(
            "Listening on Unix domain socket: {}. Waiting for client...",
            path
        );

        let (stream, _) = listener
            .accept()
            .await
            .inspect_err(|e| eprintln!("Error: Failed to accept connection: {}", e))?;

        eprintln!("Client connected");
        tracing::info!("Using Unix domain socket: {}", path);

        let (reader, writer) = tokio::io::split(stream);
        Ok((Box::pin(reader.compat()), Box::pin(writer.compat_write())))
    }

    #[cfg(windows)]
    {
        let full_path = normalize_windows_pipe(path)?;

        use tokio::net::windows::named_pipe::ServerOptions;

        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&full_path)
            .inspect_err(|e| eprintln!("Failed to create Named Pipe {}: {}", full_path, e))?;

        eprintln!("LSP server listening on Named Pipe: {}", full_path);

        server
            .connect()
            .await
            .inspect_err(|e| eprintln!("Error: Failed to accept connection: {}", e))?;

        eprintln!("Client connected");
        tracing::info!("Using Windows named pipe: {}", path);

        let (reader, writer) = tokio::io::split(server);
        Ok((Box::pin(reader.compat()), Box::pin(writer.compat_write())))
    }

    #[cfg(not(any(unix, windows)))]
    Err("Pipes are not supported on this platform".into())
}

/// Normalizes a string into a valid Windows Named Pipe path format.
///
/// If the input is a simple name, it prefixes it with `\\.\pipe\`.
/// It also validates that UNC paths contain the required `\pipe\` segment.
///
/// # Errors
///
/// Returns an error if the path contains a drive letter (indicating a file path)
/// or if a UNC path is malformed.
#[cfg(any(windows, test))]
fn normalize_windows_pipe(path: &str) -> TransportResult<String> {
    let full_path = match path {
        local if local.starts_with(r"\\.\pipe\") => local.to_string(),

        unc if unc.starts_with(r"\\") => {
            let mut components = unc.split('\\').skip(2);
            let _server = components.next();
            let pipe_segment = components.next();

            if pipe_segment == Some("pipe") {
                unc.to_string()
            } else {
                return Err(format!(
                    "Invalid UNC pipe path: '{}'. Missing or misplaced '\\pipe\\'.",
                    unc
                )
                .into());
            }
        }

        file if file.contains(':') => {
            return Err(format!("Named pipes cannot be files (like '{}').", file).into());
        }

        suffix => format!(r"\\.\pipe\{}", suffix),
    };

    Ok(full_path)
}

async fn create_stdio_transport() -> TransportResult<(LspReader, LspWriter)> {
    // Prefer truly asynchronous piped stdin/stdout without blocking tasks.
    #[cfg(unix)]
    {
        let stdin = async_lsp::stdio::PipeStdin::lock_tokio()
            .map_err(|e| format!("Failed to lock stdin: {}", e))?;
        let stdout = async_lsp::stdio::PipeStdout::lock_tokio()
            .map_err(|e| format!("Failed to lock stdout: {}", e))?;

        eprintln!("Using Stdio");
        tracing::info!("Using Stdio");

        Ok((Box::pin(stdin), Box::pin(stdout)))
    }

    // Fallback to spawn blocking read/write otherwise.
    #[cfg(not(unix))]
    {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        eprintln!("Using Stdio");
        tracing::info!("Using Stdio");

        Ok((Box::pin(stdin.compat()), Box::pin(stdout.compat_write())))
    }
}

#[cfg(unix)]
#[tokio::test]
async fn test_unix_socket_wont_delete_regular_file() {
    use tempfile::NamedTempFile;
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_str().unwrap();

    let cli = Cli {
        pipe: Some(path.to_string()),
        ..Default::default()
    };

    let result = create_transport(&cli).await;
    assert!(result.is_err());
    assert!(std::path::Path::new(path).exists());
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_windows_pipe_normalization() {
        assert_eq!(
            normalize_windows_pipe(r"lsp\protols").unwrap(),
            r"\\.\pipe\lsp\protols"
        );
        assert_eq!(
            normalize_windows_pipe(r"\\.\pipe\some\test").unwrap(),
            r"\\.\pipe\some\test"
        );

        assert!(normalize_windows_pipe(r"C:\test.proto").is_err());

        assert!(normalize_windows_pipe(r"\\server\share").is_err());

        assert!(normalize_windows_pipe(r"\\server\share\pipe").is_err());
    }
}
