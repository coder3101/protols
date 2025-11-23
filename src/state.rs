use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};
use tracing::info;

use async_lsp::lsp_types::{DocumentSymbol, ProgressParamsValue};
use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Location, OneOf, PublishDiagnosticsParams, Url,
    WorkspaceSymbol,
};
use std::sync::mpsc::Sender;
use tree_sitter::Node;
use walkdir::WalkDir;

use crate::{
    config::Config,
    nodekind::NodeKind,
    parser::{ParsedTree, ProtoParser},
};

use crate::protoc::ProtocDiagnostics;

pub struct ProtoLanguageState {
    documents: Arc<RwLock<HashMap<Url, String>>>,
    trees: Arc<RwLock<HashMap<Url, ParsedTree>>>,
    parser: Arc<Mutex<ProtoParser>>,
    parsed_workspaces: Arc<RwLock<HashSet<String>>>,
    protoc_diagnostics: Arc<Mutex<ProtocDiagnostics>>,
}

impl ProtoLanguageState {
    pub fn new() -> Self {
        ProtoLanguageState {
            documents: Default::default(),
            trees: Default::default(),
            parser: Arc::new(Mutex::new(ProtoParser::new())),
            parsed_workspaces: Arc::new(RwLock::new(HashSet::new())),
            protoc_diagnostics: Arc::new(Mutex::new(ProtocDiagnostics::new())),
        }
    }

    pub fn get_content(&self, uri: &Url) -> String {
        self.documents
            .read()
            .expect("poison")
            .get(uri)
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    pub fn get_tree(&self, uri: &Url) -> Option<ParsedTree> {
        self.trees.read().expect("poison").get(uri).cloned()
    }

    pub fn get_trees(&self) -> Vec<ParsedTree> {
        self.trees
            .read()
            .expect("poison")
            .values()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn get_trees_for_package(&self, package: &str) -> Vec<ParsedTree> {
        self.trees
            .read()
            .expect("poison")
            .values()
            .filter(|tree| {
                let content = self.get_content(&tree.uri);
                tree.get_package_name(content.as_bytes()).unwrap_or(".") == package
            })
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn find_workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let mut symbols = Vec::new();

        for tree in self.get_trees() {
            let content = self.get_content(&tree.uri);
            let doc_symbols = tree.find_document_locations(content.as_bytes());

            for doc_symbol in doc_symbols {
                self.collect_workspace_symbols(&doc_symbol, &tree.uri, query, None, &mut symbols);
            }
        }

        // Sort symbols by name and then by URI for consistent ordering
        symbols.sort_by(|a, b| {
            let name_cmp = a.name.cmp(&b.name);
            if name_cmp != std::cmp::Ordering::Equal {
                return name_cmp;
            }
            // Extract URI from location
            match (&a.location, &b.location) {
                (OneOf::Left(loc_a), OneOf::Left(loc_b)) => {
                    loc_a.uri.as_str().cmp(loc_b.uri.as_str())
                }
                _ => std::cmp::Ordering::Equal,
            }
        });

        symbols
    }

    fn collect_workspace_symbols(
        &self,
        doc_symbol: &DocumentSymbol,
        uri: &Url,
        query: &str,
        container_name: Option<String>,
        symbols: &mut Vec<WorkspaceSymbol>,
    ) {
        let symbol_name_lower = doc_symbol.name.to_lowercase();

        if query.is_empty() || symbol_name_lower.contains(query) {
            symbols.push(WorkspaceSymbol {
                name: doc_symbol.name.clone(),
                kind: doc_symbol.kind,
                tags: doc_symbol.tags.clone(),
                container_name: container_name.clone(),
                location: OneOf::Left(Location {
                    uri: uri.clone(),
                    range: doc_symbol.range,
                }),
                data: None,
            });
        }

        if let Some(children) = &doc_symbol.children {
            for child in children {
                self.collect_workspace_symbols(
                    child,
                    uri,
                    query,
                    Some(doc_symbol.name.clone()),
                    symbols,
                );
            }
        }
    }

    fn upsert_content_impl(
        &mut self,
        uri: &Url,
        content: String,
        ipath: &[PathBuf],
        depth: usize,
        parse_session: &mut HashSet<Url>,
    ) {
        // Safety: to not cause stack overflow
        if depth == 0 {
            return;
        }

        // avoid re-parsing same file incase of circular dependencies
        if parse_session.contains(uri) {
            return;
        }

        let Some(parsed) = self
            .parser
            .lock()
            .expect("poison")
            .parse(uri.clone(), content.as_bytes())
        else {
            return;
        };

        self.trees
            .write()
            .expect("posion")
            .insert(uri.clone(), parsed);

        self.documents
            .write()
            .expect("poison")
            .insert(uri.clone(), content.clone());

        parse_session.insert(uri.clone());
        let imports = self.get_owned_imports(uri, content.as_str());

        for import in imports.iter() {
            if let Some(p) = ipath.iter().map(|p| p.join(import)).find(|p| p.exists())
                && let Ok(uri) = Url::from_file_path(p.clone())
                && let Ok(content) = std::fs::read_to_string(p)
            {
                self.upsert_content_impl(&uri, content, ipath, depth - 1, parse_session);
            }
        }
    }

    fn get_owned_imports(&self, uri: &Url, content: &str) -> Vec<String> {
        self.get_tree(uri)
            .map(|t| t.get_import_paths(content.as_ref()))
            .unwrap_or_default()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn upsert_content(
        &mut self,
        uri: &Url,
        content: String,
        ipath: &[PathBuf],
        depth: usize,
    ) -> Vec<String> {
        let mut session = HashSet::new();
        self.upsert_content_impl(uri, content.clone(), ipath, depth, &mut session);

        // After content is upserted, those imports which couldn't be located
        // are flagged as import error
        self.get_tree(uri)
            .map(|t| t.get_import_paths(content.as_ref()))
            .unwrap_or_default()
            .into_iter()
            .map(ToOwned::to_owned)
            .filter(|import| !ipath.iter().any(|p| p.join(import.as_str()).exists()))
            .collect()
    }

    pub fn parse_all_from_workspace(
        &mut self,
        workspace: PathBuf,
        progress_sender: Option<Sender<ProgressParamsValue>>,
    ) {
        if self
            .parsed_workspaces
            .read()
            .expect("poison")
            .contains(workspace.to_str().unwrap_or_default())
        {
            return;
        }

        let files: Vec<_> = WalkDir::new(workspace.to_str().unwrap_or_default())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some())
            .filter(|e| e.path().extension().unwrap() == "proto")
            .collect();

        let total_files = files.len();

        for (idx, file) in files.into_iter().enumerate() {
            let path = file.path();
            if path.is_absolute()
                && path.is_file()
                && let Ok(content) = std::fs::read_to_string(path)
                && let Ok(uri) = Url::from_file_path(path)
            {
                if self.documents.read().expect("poison").contains_key(&uri) {
                    continue;
                }
                self.upsert_content(&uri, content, &[], 1);

                if let Some(sender) = &progress_sender {
                    let percentage = ((idx + 1) as f64 / total_files as f64 * 100.0) as u32;
                    let _ = sender.send(ProgressParamsValue::WorkDone(
                        async_lsp::lsp_types::WorkDoneProgress::Report(
                            async_lsp::lsp_types::WorkDoneProgressReport {
                                cancellable: None,
                                message: Some(format!(
                                    "Parsing file {} of {}",
                                    idx + 1,
                                    total_files
                                )),
                                percentage: Some(percentage),
                            },
                        ),
                    ));
                }
            }
        }

        self.parsed_workspaces
            .write()
            .expect("poison")
            .insert(workspace.to_str().unwrap_or_default().to_string());
    }

    pub fn upsert_file(
        &mut self,
        uri: &Url,
        content: String,
        ipath: &[PathBuf],
        depth: usize,
        config: &Config,
        protoc_diagnostics: bool,
    ) -> Option<PublishDiagnosticsParams> {
        info!(%uri, %depth, "upserting file");
        let diag = self.upsert_content(uri, content.clone(), ipath, depth);
        self.get_tree(uri).map(|tree| {
            let mut d = vec![];
            d.extend(tree.collect_parse_diagnostics());
            d.extend(tree.collect_import_diagnostics(content.as_ref(), diag));

            // Add protoc diagnostics if enabled
            if protoc_diagnostics
                && let Ok(protoc_diagnostics) = self.protoc_diagnostics.lock()
                && let Ok(file_path) = uri.to_file_path()
            {
                let protoc_diags = protoc_diagnostics.collect_diagnostics(
                    &config.path.protoc,
                    file_path.to_str().unwrap_or_default(),
                    &ipath
                        .iter()
                        .map(|p| p.to_str().unwrap_or_default().to_string())
                        .collect::<Vec<_>>(),
                );
                d.extend(protoc_diags);
            }

            PublishDiagnosticsParams {
                uri: tree.uri.clone(),
                diagnostics: d,
                version: None,
            }
        })
    }

    pub fn delete_file(&mut self, uri: &Url) {
        info!(%uri, "deleting file");
        self.documents.write().expect("poison").remove(uri);
        self.trees.write().expect("poison").remove(uri);
    }

    pub fn rename_file(&mut self, new_uri: &Url, old_uri: &Url) {
        info!(%new_uri, %new_uri, "renaming file");

        if let Some(v) = self.documents.write().expect("poison").remove(old_uri) {
            self.documents
                .write()
                .expect("poison")
                .insert(new_uri.clone(), v);
        }

        if let Some(mut v) = self.trees.write().expect("poison").remove(old_uri) {
            v.uri = new_uri.clone();
            self.trees
                .write()
                .expect("poison")
                .insert(new_uri.clone(), v);
        }
    }

    pub fn completion_items(&self, package: &str) -> Vec<CompletionItem> {
        let collector = |f: fn(&Node) -> bool, k: CompletionItemKind| {
            self.get_trees_for_package(package)
                .into_iter()
                .fold(vec![], |mut v, tree| {
                    let content = self.get_content(&tree.uri);
                    let t = tree.find_all_nodes(f).into_iter().map(|n| CompletionItem {
                        label: n.utf8_text(content.as_bytes()).unwrap().to_string(),
                        kind: Some(k),
                        ..Default::default()
                    });
                    v.extend(t);
                    v
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
