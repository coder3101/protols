use serde::{Deserialize, Serialize};

pub mod workspace;

fn default_clang_format_path() -> String {
    "clang-format".to_string()
}

fn default_protoc_path() -> String {
    "protoc".to_string()
}

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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct PathConfig {
    pub clang_format: String,
    pub protoc: String,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            clang_format: default_clang_format_path(),
            protoc: default_protoc_path(),
        }
    }
}
