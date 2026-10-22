use async_lsp::lsp_types::{InitializeParams, WorkDoneProgressOptions};

/// A flat, optimized summary of client editor capabilities
/// compiled once during the LSP initialization handshake.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientCapabilitiesSummary {
    /// True if the editor supports work done UI progress bars.
    pub supports_work_done_progress: bool,

    /// True if the editor allows the server to dynamically register `didChangeWatchedFiles`.
    pub supports_watched_files_dynamic_registration: bool,

    /// True if the editor can process modern directory-relative absolute glob patterns (LSP 3.17+).
    pub supports_relative_patterns: bool,
}

impl From<&InitializeParams> for ClientCapabilitiesSummary {
    fn from(value: &InitializeParams) -> Self {
        let supports_work_done_progress = Self::build_work_done_progress_options(value)
            .work_done_progress
            .unwrap_or_default();

        let capabilies = &value.capabilities;
        let watched_files_caps = capabilies
            .workspace
            .as_ref()
            .and_then(|w| w.did_change_watched_files.as_ref());
        let supports_watched_files_dynamic_registration = watched_files_caps
            .and_then(|cw| cw.dynamic_registration)
            .unwrap_or_default();
        let supports_relative_patterns = watched_files_caps
            .and_then(|cw| cw.relative_pattern_support)
            .unwrap_or(false);

        Self {
            supports_work_done_progress,
            supports_watched_files_dynamic_registration,
            supports_relative_patterns,
        }
    }
}

impl ClientCapabilitiesSummary {
    #[inline]
    pub fn build_work_done_progress_options(params: &InitializeParams) -> WorkDoneProgressOptions {
        let work_done_progress = params
            .capabilities
            .window
            .as_ref()
            .and_then(|w| w.work_done_progress);

        WorkDoneProgressOptions { work_done_progress }
    }
}
