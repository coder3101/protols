use async_lsp::Error;
use async_lsp::lsp_types::notification::{DidChangeWatchedFiles, Notification as N};
use async_lsp::lsp_types::request::{RegisterCapability, UnregisterCapability};
use async_lsp::lsp_types::{
    DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher, GlobPattern, OneOf, Registration,
    RegistrationParams, RelativePattern, Unregistration, UnregistrationParams, Url, WatchKind,
};

use super::Worker;
use crate::config::client::ClientCapabilitiesSummary;
use crate::utils::OutermostPaths;

pub(super) const FILE_WATCHER_ID: &str = "protols-proto-file-watcher";

impl Worker {
    pub(super) async fn refresh_file_watchers(&self, paths: &OutermostPaths) {
        let ClientCapabilitiesSummary {
            supports_watched_files_dynamic_registration,
            supports_relative_patterns,
            ..
        } = self.state.client_capabilities();

        if !supports_watched_files_dynamic_registration {
            return;
        }

        let kind = Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete);

        let watchers: Vec<_> = if supports_relative_patterns {
            paths
                .iter()
                .filter_map(|p| Url::from_file_path(p).ok())
                .map(|u| FileSystemWatcher {
                    glob_pattern: GlobPattern::Relative(RelativePattern {
                        base_uri: OneOf::Right(u),
                        pattern: String::from("**/*.proto"),
                    }),
                    kind,
                })
                .collect()
        } else {
            vec![FileSystemWatcher {
                glob_pattern: GlobPattern::String(String::from("**/*.proto")),
                kind,
            }]
        };

        if watchers.is_empty() {
            return;
        }

        let unregister_params = UnregistrationParams {
            unregisterations: vec![Unregistration {
                id: String::from(FILE_WATCHER_ID),
                method: DidChangeWatchedFiles::METHOD.to_string(),
            }],
        };
        let trace_error = |error: &Error| {
            tracing::warn!(
                ?error,
                "Client failed unregister dynamic file watcher capability."
            )
        };

        let _ = self
            .client
            .request::<UnregisterCapability>(unregister_params)
            .await
            .inspect_err(trace_error);

        let watcher_options = DidChangeWatchedFilesRegistrationOptions { watchers };
        let register_options = serde_json::to_value(watcher_options).unwrap_or_default();

        let registration_params = RegistrationParams {
            registrations: vec![Registration {
                id: String::from(FILE_WATCHER_ID),
                method: DidChangeWatchedFiles::METHOD.to_string(),
                register_options: Some(register_options),
            }],
        };

        let trace_error = |error: &Error| {
            tracing::error!(
                ?error,
                "CRITICAL. Client failed to register dynamic file watcher capability."
            )
        };
        let trace_ok = |_: &()| {
            tracing::info!(
                "Dynamic file watchers successfully synchronized with current workspace."
            )
        };

        self.client
            .request::<RegisterCapability>(registration_params)
            .await
            .inspect_err(trace_error)
            .inspect(trace_ok);
    }
}
