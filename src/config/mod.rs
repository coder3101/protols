use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(super) mod client;
mod workspace;

pub use workspace::WorkspaceProtoConfigs;
pub use workspace::configure_workspace_folders;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ProtolsConfig {
    pub config: Config,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub include_paths: Vec<String>,
    pub path: PathConfig,
    pub rename: RenameConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct PathConfig {
    pub clang_format: PathBuf,
    pub protoc: PathBuf,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            clang_format: "clang-format".into(),
            protoc: "protoc".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct RenameConfig {
    /// When renaming an `rpc`, also rename its convention-named
    /// `<Rpc>Request` / `<Rpc>Response` messages — and, when renaming such a
    /// message, the `rpc` and its sibling message. Opt-in: defaults to `false`
    /// since not every codebase follows the `<Rpc>Request`/`Response` naming
    /// convention.
    pub chain_rpc_request_response: bool,
}
