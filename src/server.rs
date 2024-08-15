use async_lsp::{
    lsp_types::{Url, WorkspaceFolder},
    router::Router,
    ClientSocket, ErrorCode, ResponseError,
};
use std::{collections::HashMap, fs::read_to_string, ops::ControlFlow};
use tracing::{error, info};
use walkdir::WalkDir;

use crate::parser::{ParsedTree, ProtoParser};

pub struct TickEvent;
pub struct ServerState {
    pub client: ClientSocket,
    pub counter: i32,
    pub documents: HashMap<Url, String>,
    pub trees: HashMap<Url, ParsedTree>,
    pub parser: ProtoParser,
}

impl ServerState {
    pub fn new_router(client: ClientSocket) -> Router<Self> {
        let mut router = Router::from_language_server(Self {
            client,
            counter: 0,
            documents: Default::default(),
            trees: Default::default(),
            parser: ProtoParser::new(),
        });
        router.event(Self::on_tick);
        router
    }

    fn on_tick(&mut self, _: TickEvent) -> ControlFlow<async_lsp::Result<()>> {
        self.counter += 1;
        ControlFlow::Continue(())
    }

    pub fn get_parsed_tree_and_content(
        &mut self,
        uri: &Url,
    ) -> Result<(&ParsedTree, &str), ResponseError> {
        let Some(content) = self.documents.get(uri) else {
            error!("failed to get document at {uri}");
            return Err(ResponseError::new(
                ErrorCode::INVALID_REQUEST,
                "uri was never opened",
            ));
        };

        if !self.trees.contains_key(uri) {
            let Some(parsed) = self.parser.parse(content.as_bytes()) else {
                error!("failed to parse content at {uri}");
                return Err(ResponseError::new(
                    ErrorCode::REQUEST_FAILED,
                    "ts failed to parse contents",
                ));
            };
            self.trees.insert(uri.clone(), parsed);
        }

        let parsed = self.trees.get(uri).unwrap(); // Safety: already inserted above
        Ok((parsed, content))
    }

    pub fn add_workspace_folder(&mut self, workspace: WorkspaceFolder) {
        for entry in WalkDir::new(workspace.uri.path())
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_absolute() && path.is_file() {
                let Some(ext) = path.extension() else {
                    continue;
                };

                let Ok(content) = read_to_string(path) else {
                    continue;
                };

                let Ok(uri) = Url::from_file_path(path) else {
                    continue;
                };

                if ext == "proto" {
                    self.documents.insert(uri.clone(), content);
                    let r = self.get_parsed_tree_and_content(&uri);

                    info!(
                        "workspace parse file: {}, result: {}",
                        path.display(),
                        r.is_ok()
                    );
                }
            }
        }
    }
}
