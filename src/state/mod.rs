mod definition;
mod hover;
mod rename;
mod resolve;
mod workspace_symbol;

use futures::future::Shared;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock, mpsc::Sender},
};
use tracing::info;

use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Location, OneOf, ProgressParamsValue,
    PublishDiagnosticsParams, Range, SymbolKind, SymbolTag, Url, WorkspaceSymbol,
};
use tokio::task::JoinHandle;
use tree_sitter::{Query, QueryError};
use walkdir::WalkDir;

use crate::{
    config::Config,
    document::{ProtoDocument, ProtoParser},
    model::{ElementKind, generate_metamodel_query},
    protoc::collect_diagnostics,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocumentVersion(pub i32);

#[derive(Clone)]
pub enum CacheEntry {
    Ready {
        version: DocumentVersion,
        document: ProtoDocument,
    },
    Pending {
        version: DocumentVersion,
        future: Shared<JoinHandle<Arc<ProtoDocument>>>,
    },
    Dirty,
}

pub struct ProtoLanguageState {
    pub cache: Arc<RwLock<HashMap<Url, CacheEntry>>>,
    sources: Arc<RwLock<HashMap<Url, String>>>,
    documents: Arc<RwLock<HashMap<Url, ProtoDocument>>>,
    parser: Arc<Mutex<ProtoParser>>,
    parsed_workspaces: Arc<RwLock<HashSet<String>>>,
    pub metamodel_query: Arc<Query>,
}

impl ProtoLanguageState {
    pub fn new() -> Self {
        let language: tree_sitter::Language = tree_sitter_proto::LANGUAGE.into();
        let trace_error = |e: &QueryError| {
            tracing::error!(
                "Critical SCM error: Failed to compile embedded Tree-sitter query for metadata extraction. Details: {:?}",
                e
            );
        };

        let metamodel_query = Query::new(&language, &generate_metamodel_query())
            .inspect_err(trace_error)
            .expect("Tree-sitter query compilation failed");

        Self {
            cache: Arc::default(),
            sources: Arc::default(),
            documents: Arc::default(),
            parser: Arc::new(Mutex::new(ProtoParser::new())),
            parsed_workspaces: Arc::new(RwLock::new(HashSet::new())),
            metamodel_query: Arc::new(metamodel_query),
        }
    }

    pub fn get_content(&self, uri: &Url) -> String {
        self.sources
            .read()
            .expect("poison")
            .get(uri)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_document(&self, uri: &Url) -> Option<ProtoDocument> {
        self.documents.read().expect("poison").get(uri).cloned()
    }

    pub fn get_documents(&self) -> Vec<ProtoDocument> {
        self.documents
            .read()
            .expect("poison")
            .values()
            .map(ToOwned::to_owned)
            .collect()
    }

    pub fn get_documents_for_package(&self, package: &str) -> Vec<ProtoDocument> {
        self.documents
            .read()
            .expect("poison")
            .values()
            .filter(|document| document.package == package)
            .map(ToOwned::to_owned)
            .collect()
    }

    /// Runs a fast, pure-Rust substring match over the cached metamodel pool
    /// populated during startup indexing.
    ///
    /// This deliberately avoids re-parsing the workspace or rebuilding the
    /// hierarchical [`DocumentSymbol`] document on every request. Instead it scans
    /// the flat, already-indexed [`ModelElement`] registry and resolves each
    /// candidate's container name by walking the in-memory parent links.
    pub fn find_workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let query = query.to_lowercase();
        let mut symbols = Vec::new();

        for document in self.get_documents() {
            for element in &document.elements {
                if matches!(element.kind, ElementKind::Import { .. }) {
                    continue;
                }

                let name_lower = element.meta.name.to_lowercase();
                if !query.is_empty() && !name_lower.contains(&query) {
                    continue;
                }

                let container_name = element
                    .parent_id
                    .and_then(|parent_id| document.elements.get(parent_id))
                    .map(|parent| parent.meta.name.clone());

                let range =
                    element
                        .meta
                        .documentation
                        .first()
                        .map_or(element.meta.range, |comment| Range {
                            start: comment.range.start,
                            end: element.meta.range.end,
                        });

                symbols.push(WorkspaceSymbol {
                    name: element.meta.name.clone(),
                    kind: SymbolKind::from(&element.kind),
                    tags: element
                        .kind
                        .is_deprecated()
                        .then(|| vec![SymbolTag::DEPRECATED]),
                    container_name,
                    location: OneOf::Left(Location {
                        uri: document.uri.clone(),
                        range,
                    }),
                    data: None,
                });
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

    fn upsert_content_impl(
        &mut self,
        uri: &Url,
        content: &str,
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

        let Some(parsed) = self.parser.lock().expect("poison").parse(
            uri.clone(),
            content.as_bytes(),
            &self.metamodel_query,
        ) else {
            return;
        };

        self.documents
            .write()
            .expect("posion")
            .insert(uri.clone(), parsed);

        self.sources
            .write()
            .expect("poison")
            .insert(uri.clone(), content.to_string());

        parse_session.insert(uri.clone());
        let imports = self.get_owned_imports(uri, content);

        for import in &imports {
            if let Some(p) = ipath.iter().map(|p| p.join(import)).find(|p| p.exists())
                && let Ok(uri) = Url::from_file_path(p.clone())
                && let Ok(content) = std::fs::read_to_string(p)
            {
                self.upsert_content_impl(&uri, &content, ipath, depth - 1, parse_session);
            }
        }
    }

    fn get_owned_imports(&self, uri: &Url, _content: &str) -> Vec<String> {
        self.get_document(uri)
            .map(|t| t.import_paths())
            .unwrap_or_default()
    }

    pub fn upsert_content(
        &mut self,
        uri: &Url,
        content: &str,
        ipath: &[PathBuf],
        depth: usize,
    ) -> Vec<String> {
        let mut session = HashSet::new();
        self.upsert_content_impl(uri, content, ipath, depth, &mut session);

        // After content is upserted, those imports which couldn't be located
        // are flagged as import error
        self.get_document(uri)
            .map(|t| t.import_paths())
            .unwrap_or_default()
            .into_iter()
            .filter(|import| !ipath.iter().any(|p| p.join(import.as_str()).exists()))
            .collect()
    }

    pub fn parse_all_from_workspace(
        &mut self,
        workspace: &Path,
        progress_sender: Option<&Sender<ProgressParamsValue>>,
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
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                if let Some(ext) = e.path().extension() {
                    return ext == "proto";
                }
                false
            })
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
                self.upsert_content(&uri, &content, &[], 1);

                if let Some(sender) = &progress_sender {
                    let percentage =
                        u32::try_from((idx + 1 / total_files) * 100).unwrap_or_default();
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
        content: &str,
        ipath: &[PathBuf],
        depth: usize,
        config: &Config,
        protoc_diagnostics: bool,
    ) -> Option<PublishDiagnosticsParams> {
        info!(%uri, %depth, "upserting file");
        let diag = self.upsert_content(uri, content, ipath, depth);
        let diag_slice: Vec<&str> = diag.iter().map(String::as_str).collect();
        self.get_document(uri).map(|document| {
            let mut d = vec![];
            d.extend(document.collect_parse_diagnostics());
            d.extend(document.collect_import_diagnostics(diag_slice.as_slice()));

            // Add protoc diagnostics if enabled
            if protoc_diagnostics && let Ok(file_path) = uri.to_file_path() {
                let protoc_diags = collect_diagnostics(
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
                uri: document.uri.clone(),
                diagnostics: d,
                version: None,
            }
        })
    }

    pub fn delete_file(&mut self, uri: &Url) {
        info!(%uri, "deleting file");
        self.sources.write().expect("poison").remove(uri);
        self.documents.write().expect("poison").remove(uri);
    }

    pub fn rename_file(&mut self, new_uri: &Url, old_uri: &Url) {
        info!(%new_uri, %old_uri, "renaming file");

        let content = self.sources.write().expect("poison").remove(old_uri);
        if let Some(v) = content {
            self.sources
                .write()
                .expect("poison")
                .insert(new_uri.clone(), v);
        }

        let mut document = self.documents.write().expect("poison").remove(old_uri);
        if let Some(ref mut v) = document {
            v.uri = new_uri.clone();
        }
        if let Some(v) = document {
            self.documents
                .write()
                .expect("poison")
                .insert(new_uri.clone(), v);
        }
    }

    pub fn completion_items_for_document(&self, url: &Url) -> Vec<CompletionItem> {
        let collector = |f: fn(&ElementKind) -> bool, k: CompletionItemKind| {
            self.get_document(url)
                .map(|document| {
                    document
                        .elements
                        .iter()
                        .filter(|e| f(&e.kind))
                        .map(|e| CompletionItem {
                            label: format!(".{}.{}", document.package, e.meta.name),
                            kind: Some(k),
                            ..Default::default()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };

        let mut result = collector(is_enum_kind, CompletionItemKind::ENUM);
        result.extend(collector(is_message_kind, CompletionItemKind::STRUCT));
        // Better ways to dedup, but who cares?...
        result.sort_by_key(|k| k.label.clone());
        result.dedup_by_key(|k| k.label.clone());
        result
    }

    pub fn completion_items_for_package(&self, package: &str) -> Vec<CompletionItem> {
        let collector =
            |f: fn(&ElementKind) -> bool, k: CompletionItemKind| {
                self.get_documents_for_package(package).into_iter().fold(
                    vec![],
                    |mut v, document| {
                        let t = document.elements.iter().filter(|e| f(&e.kind)).map(|e| {
                            CompletionItem {
                                label: e.meta.name.clone(),
                                kind: Some(k),
                                ..Default::default()
                            }
                        });
                        v.extend(t);
                        v
                    },
                )
            };

        let mut result = collector(is_enum_kind, CompletionItemKind::ENUM);
        result.extend(collector(is_message_kind, CompletionItemKind::STRUCT));
        // Better ways to dedup, but who cares?...
        result.sort_by_key(|k| k.label.clone());
        result.dedup_by_key(|k| k.label.clone());
        result
    }
}

fn is_enum_kind(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::Enum { .. })
}

fn is_message_kind(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::Message { .. })
}

#[cfg(test)]
mod test {
    use super::*;
    use async_lsp::lsp_types::Url;
    use std::path::PathBuf;

    fn uri(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn setup_state() -> ProtoLanguageState {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];

        state.upsert_content(
            &uri("file:///test.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book { string title = 1; }\nenum Color { RED = 0; }\n",
            ipath,
            1,
        );
        state.upsert_content(
            &uri("file:///other.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Author { string name = 1; }\n",
            ipath,
            1,
        );
        state.upsert_content(
            &uri("file:///diff.proto"),
            "syntax = \"proto3\";\npackage com.other;\nmessage Foo { int32 bar = 1; }\n",
            ipath,
            1,
        );
        state
    }

    #[test]
    fn test_get_content() {
        let state = setup_state();
        assert_eq!(
            state.get_content(&uri("file:///test.proto")),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book { string title = 1; }\nenum Color { RED = 0; }\n"
        );
        assert_eq!(state.get_content(&uri("file:///nonexistent.proto")), "");
    }

    #[test]
    fn test_get_document() {
        let state = setup_state();
        assert!(state.get_document(&uri("file:///test.proto")).is_some());
        assert!(
            state
                .get_document(&uri("file:///nonexistent.proto"))
                .is_none()
        );
    }

    #[test]
    fn test_get_documents() {
        let state = setup_state();
        let documents = state.get_documents();
        assert_eq!(documents.len(), 3);
    }

    #[test]
    fn test_get_documents_for_package() {
        let state = setup_state();
        let test_documents = state.get_documents_for_package("com.test");
        assert_eq!(test_documents.len(), 2);

        let other_documents = state.get_documents_for_package("com.other");
        assert_eq!(other_documents.len(), 1);

        let empty_documents = state.get_documents_for_package("com.nonexistent");
        assert!(empty_documents.is_empty());
    }

    #[test]
    fn test_document_completion_items() {
        let state = setup_state();
        let items = state.completion_items_for_document(&uri("file:///test.proto"));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&".com.test.Book"));
        assert!(labels.contains(&".com.test.Color"));
        assert!(!labels.contains(&".com.test.Author"));
    }

    #[test]
    fn test_completion_excludes_fields_enum_values_and_imports() {
        // Completion offers only type-level symbols (messages/enums); fields,
        // enum values, and imports must never leak in.
        let state = setup_state();
        let items = state.completion_items_for_document(&uri("file:///test.proto"));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(!labels.contains(&".com.test.Book.title"));
        assert!(!labels.contains(&".com.test.Color.RED"));
        assert!(!labels.contains(&".com.test.import"));
    }

    #[test]
    fn test_document_completion_empty_for_missing_document() {
        let state = setup_state();
        assert!(
            state
                .completion_items_for_document(&uri("file:///missing.proto"))
                .is_empty()
        );
    }

    #[test]
    fn test_package_completion_items() {
        let state = setup_state();
        let items = state.completion_items_for_package("com.test");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Book"));
        assert!(labels.contains(&"Color"));
        assert!(labels.contains(&"Author"));
        assert!(!labels.contains(&"Foo"));

        let other_items = state.completion_items_for_package("com.other");
        let other_labels: Vec<&str> = other_items.iter().map(|i| i.label.as_str()).collect();
        assert!(other_labels.contains(&"Foo"));
        assert!(!other_labels.contains(&"Book"));
    }

    #[test]
    fn test_package_completion_items_empty_package() {
        let state = setup_state();
        let items = state.completion_items_for_package("com.nonexistent");
        assert!(items.is_empty());
    }

    #[test]
    fn test_find_workspace_symbols_empty_query() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Book"));
        assert!(names.contains(&"Author"));
        assert!(names.contains(&"Color"));
        assert!(names.contains(&"Foo"));
    }

    #[test]
    fn test_find_workspace_symbols_partial_query() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("oo");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Book"));
        assert!(names.contains(&"Foo"));
        assert!(!names.contains(&"Author"));
    }

    #[test]
    fn test_find_workspace_symbols_case_insensitive() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("book");
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Book"));
    }

    #[test]
    fn test_find_workspace_symbols_no_match() {
        let state = setup_state();
        let symbols = state.find_workspace_symbols("zzzzz");
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_delete_file() {
        let mut state = setup_state();
        let test_uri = uri("file:///test.proto");
        assert!(state.get_document(&test_uri).is_some());
        state.delete_file(&test_uri);
        assert!(state.get_document(&test_uri).is_none());
        assert_eq!(state.get_content(&test_uri), "");
    }

    #[test]
    fn test_rename_file() {
        let mut state = setup_state();
        let old_uri = uri("file:///test.proto");
        let new_uri = uri("file:///renamed.proto");

        assert!(state.get_document(&old_uri).is_some());
        assert!(state.get_document(&new_uri).is_none());

        state.rename_file(&new_uri, &old_uri);

        assert!(state.get_document(&old_uri).is_none());
        assert!(state.get_document(&new_uri).is_some());
        assert_eq!(
            state.get_content(&new_uri),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book { string title = 1; }\nenum Color { RED = 0; }\n"
        );
    }

    #[test]
    fn test_upsert_content_tracks_unresolved_imports() {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];
        let unresolved = state.upsert_content(
            &uri("file:///importing.proto"),
            "syntax = \"proto3\";\nimport \"nonexistent.proto\";\npackage com.test;\n",
            ipath,
            1,
        );
        assert_eq!(unresolved, vec!["nonexistent.proto"]);
    }

    #[test]
    fn test_upsert_content_resolved_imports() {
        let mut state = ProtoLanguageState::new();
        let dir = tempfile::tempdir().unwrap();
        let dep_path = dir.path().join("dep.proto");
        std::fs::write(&dep_path, "syntax = \"proto3\";\npackage com.dep;\n").unwrap();
        let ipath = vec![dir.path().to_path_buf()];

        let unresolved = state.upsert_content(
            &uri("file:///main.proto"),
            "syntax = \"proto3\";\nimport \"dep.proto\";\npackage com.main;\n",
            &ipath,
            1,
        );
        assert!(unresolved.is_empty());
        assert!(state.get_document(&uri("file:///main.proto")).is_some());
    }

    #[test]
    fn test_upsert_content_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        let a_path = dir.path().join("a.proto");
        let b_path = dir.path().join("b.proto");
        std::fs::write(
            &a_path,
            "syntax = \"proto3\";\nimport \"b.proto\";\npackage com.a;\n",
        )
        .unwrap();
        std::fs::write(
            &b_path,
            "syntax = \"proto3\";\nimport \"a.proto\";\npackage com.b;\n",
        )
        .unwrap();
        let ipath = vec![dir.path().to_path_buf()];

        // depth=0 should not parse anything
        let mut state0 = ProtoLanguageState::new();
        state0.upsert_content(
            &uri("file:///a.proto"),
            std::fs::read_to_string(&a_path).unwrap().as_str(),
            &ipath,
            0,
        );
        assert!(state0.get_document(&uri("file:///a.proto")).is_none());

        // depth=1 should parse a.proto but not follow imports
        let mut state1 = ProtoLanguageState::new();
        state1.upsert_content(
            &uri("file:///a.proto"),
            std::fs::read_to_string(&a_path).unwrap().as_str(),
            &ipath,
            1,
        );
        assert!(state1.get_document(&uri("file:///a.proto")).is_some());
        assert!(state1.get_document(&uri("file:///b.proto")).is_none());
    }

    #[test]
    fn test_parse_all_from_workspace() {
        let mut state = ProtoLanguageState::new();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.proto"),
            "syntax = \"proto3\";\npackage com.a;\nmessage A {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("b.proto"),
            "syntax = \"proto3\";\npackage com.b;\nmessage B {}\n",
        )
        .unwrap();
        // Non-proto file should be ignored
        std::fs::write(dir.path().join("notes.txt"), "hello").unwrap();

        state.parse_all_from_workspace(dir.path(), None);
        assert_eq!(state.get_documents().len(), 2);

        // Second call should be idempotent
        state.parse_all_from_workspace(dir.path(), None);
        assert_eq!(state.get_documents().len(), 2);
    }

    #[test]
    fn test_upsert_file_returns_diagnostics() {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];
        let result = state.upsert_file(
            &uri("file:///test.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book {}\n",
            ipath,
            1,
            &Config::default(),
            false,
        );
        assert!(result.is_some());
        let params = result.unwrap();
        assert_eq!(params.uri.as_str(), "file:///test.proto");
        // Should have no diagnostics for valid proto
        assert!(params.diagnostics.is_empty());
    }

    #[test]
    fn test_upsert_file_returns_parse_diagnostics() {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];
        let result = state.upsert_file(
            &uri("file:///bad.proto"),
            "syntax = \"proto3\";\npackage com.test;\nmessage Book { invalid syntax here }\n",
            ipath,
            1,
            &Config::default(),
            false,
        );
        assert!(result.is_some());
        let params = result.unwrap();
        assert!(
            !params.diagnostics.is_empty(),
            "expected parse diagnostics for invalid proto"
        );
    }
}
