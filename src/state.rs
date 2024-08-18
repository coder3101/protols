use std::{collections::HashMap, fs::read_to_string};
use tracing::{error, info};

use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, PublishDiagnosticsParams, Url, WorkspaceFolder,
};
use tree_sitter::Node;
use walkdir::WalkDir;

use crate::{
    nodekind::NodeKind,
    parser::{ParsedTree, ProtoParser},
};

pub struct ProtoLanguageState {
    documents: HashMap<Url, String>,
    pub trees: HashMap<Url, ParsedTree>,
    parser: ProtoParser,
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

    pub fn get_tree(&self, uri: &Url) -> Option<&ParsedTree> {
        self.trees.get(uri)
    }

    pub fn get_trees_for_package(&self, package: &str) -> Vec<&ParsedTree> {
        self.trees
            .values()
            .filter(|tree| {
                let content = self.get_content(&tree.uri);
                tree.get_package_name(content.as_bytes())
                    .unwrap_or_default()
                    == package
            })
            .collect()
    }

    pub fn upsert_content(&mut self, uri: &Url, content: String) -> bool {
        if let Some(parsed) = self.parser.parse(uri.clone(), content.as_bytes()) {
            self.trees.insert(uri.clone(), parsed);
            self.documents.insert(uri.clone(), content);
            true
        } else {
            error!(uri=%uri, "failed to parse content");
            false
        }
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
                    let r = self.upsert_content(&uri, content);
                    info!("workspace parse file: {}, result: {}", path.display(), r);
                }
            }
        }
    }

    pub fn upsert_file(&mut self, uri: &Url, content: String) -> Option<PublishDiagnosticsParams> {
        info!(uri=%uri, "upserting file");
        self.upsert_content(uri, content);
        self.get_tree(uri).map(|tree| tree.collect_parse_errors())
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

    pub fn completion_items(&self, package: &str) -> Vec<CompletionItem> {
        let collector = |f: fn(&Node) -> bool, k: CompletionItemKind| {
            self.get_trees_for_package(package)
                .into_iter()
                .fold(vec![], |mut v, tree| {
                    let content = self.get_content(&tree.uri);
                    let t = tree.filter_nodes(f).into_iter().map(|n| CompletionItem {
                        label: n.utf8_text(content.as_bytes()).unwrap().to_string(),
                        kind: Some(k),
                        ..Default::default()
                    });
                    v.extend(t);
                    return v;
                })
        };

        let mut result = collector(NodeKind::is_enum_name, CompletionItemKind::ENUM);
        result.extend(collector(
            NodeKind::is_message_name,
            CompletionItemKind::STRUCT,
        ));
        // Better ways to dedup, but who cares?...
        result.sort_by_key(|k| k.label.clone());
        result.dedup_by_key(|k| k.label.clone());
        result
    }
}
