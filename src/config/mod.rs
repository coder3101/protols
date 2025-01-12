use serde::{Deserialize, Serialize};

pub mod workspace;

fn default_clang_format_path() -> String {
    "clang-format".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ProtolsConfig {
    pub config: Config,
    pub formatter: FormatterConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FormatterConfig {
    #[serde(default = "default_clang_format_path")]
    pub clang_format_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub include_paths: Vec<String>,
    pub single_file_mode: bool,
    pub disable_parse_diagnostics: bool,
    pub experimental: ExperimentalConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct ExperimentalConfig {
    pub use_protoc_diagnostics: bool,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            clang_format_path: default_clang_format_path(),
        }
    }
}
