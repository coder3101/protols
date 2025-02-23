use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};
use tracing::info;

use async_lsp::lsp_types::{CompletionItem, CompletionItemKind, PublishDiagnosticsParams, Url};
use tree_sitter::Node;

use crate::{
    nodekind::NodeKind,
    parser::{ParsedTree, ProtoParser},
};

pub struct ProtoLanguageState {
    documents: Arc<RwLock<HashMap<Url, String>>>,
    trees: Arc<RwLock<HashMap<Url, ParsedTree>>>,
    parser: Arc<Mutex<ProtoParser>>,
}

impl ProtoLanguageState {
    pub fn new() -> Self {
        ProtoLanguageState {
            documents: Default::default(),
            trees: Default::default(),
            parser: Arc::new(Mutex::new(ProtoParser::new())),
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
                tree.get_package_name(content.as_bytes())
                    .unwrap_or_default()
                    == package
            })
            .map(ToOwned::to_owned)
            .collect()
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
            if let Some(p) = ipath.iter().map(|p| p.join(import)).find(|p| p.exists()) {
                if let Ok(uri) = Url::from_file_path(p.clone()) {
                    if let Ok(content) = std::fs::read_to_string(p) {
                        self.upsert_content_impl(&uri, content, ipath, depth - 1, parse_session);
                    }
                }
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

    // #[allow(unused)]
    // pub fn add_workspace_folder_async(
    //     &mut self,
    //     workspace: WorkspaceFolder,
    //     tx: Sender<ProgressParamsValue>,
    // ) {
    //     let parser = self.parser.clone();
    //     let tree = self.trees.clone();
    //     let docs = self.documents.clone();
    //
    //     let begin = ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
    //         title: String::from("indexing"),
    //         cancellable: Some(false),
    //         percentage: Some(0),
    //         ..Default::default()
    //     }));
    //
    //     if let Err(e) = tx.send(begin) {
    //         error!(error=%e, "failed to send work begin progress");
    //     }
    //
    //     thread::spawn(move || {
    //         let files: Vec<_> = WalkDir::new(workspace.uri.path())
    //             .into_iter()
    //             .filter_map(|e| e.ok())
    //             .filter(|e| e.path().extension().is_some())
    //             .filter(|e| e.path().extension().unwrap() == "proto")
    //             .collect();
    //
    //         let total_files = files.len();
    //         let mut current = 0;
    //
    //         for file in files.into_iter() {
    //             let path = file.path();
    //             if path.is_absolute() && path.is_file() {
    //                 let Ok(content) = read_to_string(path) else {
    //                     continue;
    //                 };
    //
    //                 let Ok(uri) = Url::from_file_path(path) else {
    //                     continue;
    //                 };
    //
    //                 Self::upsert_content_impl(
    //                     parser.lock().expect("poison"),
    //                     &uri,
    //                     content,
    //                     docs.write().expect("poison"),
    //                     tree.write().expect("poison"),
    //                 );
    //
    //                 current += 1;
    //
    //                 let report = ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
    //                     WorkDoneProgressReport {
    //                         cancellable: Some(false),
    //                         message: Some(path.display().to_string()),
    //                         percentage: Some((current * 100 / total_files) as u32),
    //                     },
    //                 ));
    //
    //                 if let Err(e) = tx.send(report) {
    //                     error!(error=%e, "failed to send work report progress");
    //                 }
    //             }
    //         }
    //         let report =
    //             ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
    //                 message: Some(String::from("completed")),
    //             }));
    //
    //         info!(len = total_files, "workspace file parsing completed");
    //         if let Err(e) = tx.send(report) {
    //             error!(error=%e, "failed to send work completed result");
    //         }
    //     });
    // }

    pub fn upsert_file(
        &mut self,
        uri: &Url,
        content: String,
        ipath: &[PathBuf],
        depth: usize,
    ) -> Option<PublishDiagnosticsParams> {
        info!(%uri, %depth, "upserting file");
        let diag = self.upsert_content(uri, content.clone(), ipath, depth);
        self.get_tree(uri).map(|tree| {
            let diag = tree.collect_import_diagnostics(content.as_ref(), diag);
            let mut d = tree.collect_parse_diagnostics();
            d.extend(diag);
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
