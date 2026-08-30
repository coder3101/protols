use std::ops::ControlFlow;
use std::{collections::HashSet, path::PathBuf};

use async_lsp::Error;
use async_lsp::lsp_types::InitializedParams;

use crate::server::ProtoLanguageServer;

impl ProtoLanguageServer {
    pub(super) fn initialized(
        &mut self,
        _params: InitializedParams,
    ) -> ControlFlow<Result<(), Error>> {
        let paths: HashSet<PathBuf> = self
            .configs
            .get_outermost_workspaces()
            .into_iter()
            .filter_map(|url| url.to_file_path().ok())
            .collect();

        if paths.is_empty() {
            return ControlFlow::Continue(());
        }

        let cancel_token = self.shutdown_cancel_token.clone();
        let query = self.state.metamodel_query.clone();
        let cache = self.state.cache.clone();

        tokio::spawn(async move {
            use crate::workspace_symbol::warmup_workspaces;

            let warmup_future = warmup_workspaces(paths, query, cache, cancel_token.clone());

            cancel_token.run_until_cancelled(warmup_future).await;
        });

        ControlFlow::Continue(())
    }
}
