use std::{collections::HashMap, fs::read_to_string};
use tracing::{error, info};

use async_lsp::{
    lsp_types::{PublishDiagnosticsParams, Url, WorkspaceFolder},
    ErrorCode, ResponseError,
};
use walkdir::WalkDir;

use crate::parser::{ParsedTree, ProtoParser};

pub struct ProtoLanguageState {
    pub documents: HashMap<Url, String>,
    pub trees: HashMap<Url, ParsedTree>,
    pub parser: ProtoParser,
}

impl ProtoLanguageState {
    pub fn new() -> Self {
        ProtoLanguageState {
            documents: Default::default(),
            trees: Default::default(),
            parser: ProtoParser::new(),
        }
    }

    pub fn get_content(&self, uri: &Url) -> &str {
        self.documents
            .get(uri)
            .map(|s| s.as_str())
            .unwrap_or_default()
    }

    pub fn get_trees_for_package(&self, package: &str) -> Vec<&ParsedTree> {
        self.trees.values().filter(|tree| {
            let content = self.get_content(&tree.uri);
            tree.get_package_name(content.as_bytes()).unwrap_or_default() == package
        }).collect()
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
            let Some(parsed) = self.parser.parse(uri.clone(), content.as_bytes()) else {
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

    pub fn upsert_file(&mut self, uri: &Url, content: String) -> Option<PublishDiagnosticsParams> {
        info!(uri=%uri, "upserting file");

        let Some(tree) = self.parser.parse(uri.clone(), &content) else {
            error!(uri=%uri, "failed to parse content");
            return None;
        };

        self.documents.insert(uri.clone(), content);
        let diagnostics = tree.collect_parse_errors();

        self.trees.insert(uri.clone(), tree);
        Some(diagnostics)
    }

    pub fn delete_file(&mut self, uri: &Url) {
        info!(uri=%uri, "deleting file");

        self.documents.remove(uri);
        self.trees.remove(uri);
    }

    pub fn rename_file(&mut self, new_uri: &Url, old_uri: &Url) {
        info!(new_uri=%new_uri, old_uri=%new_uri, "renaming file");

        if let Some(v) = self.documents.remove(old_uri) {
            self.documents.insert(new_uri.clone(), v);
        }

        if let Some(mut v) = self.trees.remove(old_uri) {
            v.uri = new_uri.clone();
            self.trees.insert(new_uri.clone(), v);
        }
    }
}