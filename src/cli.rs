use clap::Parser;
use const_format::concatcp;

use crate::FALLBACK_INCLUDE_PATH;

const BUILD_INFO: &str = concatcp!(
    "fallback include path: ",
    match FALLBACK_INCLUDE_PATH {
        Some(path) => path,
        None => "not set",
    }
);

/// Command-line arguments for the protols language server.
///
/// This structure defines the available configuration options, including
/// file discovery paths and various communication transports (Stdio, TCP, Pipes).
#[derive(Parser, Debug, Default)]
#[command(
    author,
    version = concatcp!(
        env!("CARGO_PKG_VERSION"),
        "\n",
        BUILD_INFO
    ),
    about,
    long_about = None
)]
pub struct Cli {
    /// Include paths for proto files, comma-separated (can be used multiple times)
    #[arg(short, long, value_delimiter = ',')]
    pub include_paths: Option<Vec<String>>,

    /// Use stdin/stdout for communication (default)
    #[arg(long, group = "transport", help_heading = "Transport")]
    pub stdio: bool,

    /// Use TCP communication with a specific address and port.
    /// Examples: "192.168.1.10:5005" or "0.0.0.0:5005"
    #[arg(
        long,
        value_name = "ADDR",
        group = "transport",
        help_heading = "Transport"
    )]
    pub socket: Option<String>,

    /// Use TCP communication on localhost with a specific port.
    /// Example: "5005"
    #[arg(
        long,
        value_name = "PORT",
        group = "transport",
        help_heading = "Transport"
    )]
    pub port: Option<u16>,

    /// Use Unix domain socket (Linux/macOS) or Named Pipe (Windows).
    /// Examples: "/tmp/protols.sock" or "protols-pipe" (Windows)
    #[arg(
        long,
        value_name = "PATH",
        group = "transport",
        help_heading = "Transport"
    )]
    pub pipe: Option<String>,
}

impl Cli {
    /// Returns a list of filesystem paths for proto file discovery.
    ///
    /// This method collects all values from the `--include-paths` flags (including
    /// multiple occurrences and comma-separated values) and converts them into
    /// a vector of [std::path::PathBuf]. Returns an empty vector if no paths are provided.
    pub fn get_include_paths(&self) -> Vec<std::path::PathBuf> {
        self.include_paths
            .as_ref()
            .map(|ic| ic.iter().map(std::path::PathBuf::from).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Test with no arguments
        let args = vec!["protols"];
        let cli = Cli::try_parse_from(args).expect("Should parse empty args");
        assert!(cli.get_include_paths().is_empty());

        // Test with include paths
        let args = vec!["protols", "--include-paths=/path1,/path2"];
        let cli = Cli::try_parse_from(args).expect("Should parse long flag");
        let paths = cli.get_include_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].to_str().unwrap(), "/path1");
        assert_eq!(paths[1].to_str().unwrap(), "/path2");

        // Test with short form
        let args = vec!["protols", "-i", "/path1,/path2"];
        let cli = Cli::try_parse_from(args).expect("Should parse short flag");
        let paths = cli.get_include_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], std::path::PathBuf::from("/path1"));
        assert_eq!(paths[1], std::path::PathBuf::from("/path2"));

        // Test include path multiple occurrences merging
        let args = vec!["protols", "-i", "/path2", "-i", "/path1"];
        let cli = Cli::try_parse_from(args).unwrap();
        let paths = cli.get_include_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], std::path::PathBuf::from("/path2"));
        assert_eq!(paths[1], std::path::PathBuf::from("/path1"));

        // Windows-style paths
        let args = vec!["protols", "-i", r"C:\proto\include,D:\shared"];
        let cli = Cli::try_parse_from(args).unwrap();
        let paths = cli.get_include_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].to_str().unwrap(), r"C:\proto\include");
        assert_eq!(paths[1].to_str().unwrap(), r"D:\shared");
    }

    #[test]
    fn test_get_include_paths_transformation() {
        let args = vec!["protols", "-i", "rel/path,/abs/path"];
        let cli = Cli::parse_from(args);
        let paths = cli.get_include_paths();

        assert_eq!(paths.len(), 2);
        assert!(paths[0].is_relative());
        assert!(paths[1].is_absolute());
    }

    #[test]
    fn test_transport_conflict() {
        let args = vec!["protols", "--port", "5005", "--socket", "172.16.0.15:5005"];

        let result = Cli::try_parse_from(args);

        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn test_port_and_socket() {
        let args = vec!["protols", "--port", "7301"];
        let cli = Cli::parse_from(args);
        assert_eq!(cli.port, Some(7301));

        let args = vec!["protols", "--socket", "192.168.1.20:7301"];
        let cli = Cli::parse_from(args);
        assert_eq!(cli.socket.as_deref(), Some("192.168.1.20:7301"));
    }

    #[test]
    fn test_default_is_empty() {
        let args = vec!["protols"];
        let cli = Cli::try_parse_from(args).unwrap();

        assert!(!cli.stdio);
        assert!(cli.port.is_none());
        assert!(cli.socket.is_none());
        assert!(cli.pipe.is_none());
    }
}
