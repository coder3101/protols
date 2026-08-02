use std::ops::ControlFlow;
use std::{fs::read_to_string, path::PathBuf};
use tracing::{error, info, warn};

use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    CreateFilesParams, DeleteFilesParams, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentFormattingParams, DocumentRangeFormattingParams,
    DocumentSymbolParams, DocumentSymbolResponse, Documentation, FileOperationFilter,
    FileOperationPattern, FileOperationPatternKind, FileOperationRegistrationOptions,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, Location, MarkupContent, MarkupKind, OneOf,
    PrepareRenameResponse, ReferenceParams, RenameFilesParams, RenameOptions, RenameParams,
    ServerCapabilities, ServerInfo, SetTraceParams, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url, WorkDoneProgressOptions,
    WorkspaceEdit, WorkspaceFileOperationsServerCapabilities, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities, WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use async_lsp::{Error, LanguageClient, ResponseError};
use futures::future::BoxFuture;
use serde_json::Value;

use crate::formatter::ProtoFormatter;
use crate::server::ProtoLanguageServer;
use crate::{docs, log};

impl ProtoLanguageServer {
    pub(super) fn initialize(
        &mut self,
        params: InitializeParams,
    ) -> BoxFuture<'static, Result<InitializeResult, ResponseError>> {
        let (cname, version) = params
            .client_info
            .as_ref()
            .map_or(("<unknown>", None), |c| {
                (c.name.as_str(), c.version.as_deref())
            });

        let cversion = version.unwrap_or("<unknown>");

        info!("Connected with client {cname} {cversion}");

        // Parse initialization options for include paths
        if let Some(init_options) = &params.initialization_options
            && let Some(include_paths) = parse_init_include_paths(init_options)
        {
            info!(
                "Setting include paths from initialization options: {:?}",
                include_paths
            );
            self.configs.set_init_include_paths(include_paths);
        }

        let file_operation_filers = vec![FileOperationFilter {
            scheme: Some(String::from("file")),
            pattern: FileOperationPattern {
                glob: String::from("**/*.proto"),
                matches: Some(FileOperationPatternKind::File),
                ..Default::default()
            },
        }];

        let file_registration_option = FileOperationRegistrationOptions {
            filters: file_operation_filers.clone(),
        };

        let mut workspace_capabilities = None;

        if let Some(folders) = params.workspace_folders {
            for workspace in folders {
                info!("Workspace folder: {workspace:?}");
                self.configs.add_workspace(&workspace);
            }
            workspace_capabilities = Some(WorkspaceServerCapabilities {
                workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                    supported: Some(true),
                    ..Default::default()
                }),

                file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                    did_create: Some(file_registration_option.clone()),
                    did_rename: Some(file_registration_option.clone()),
                    did_delete: Some(file_registration_option.clone()),
                    ..Default::default()
                }),
            });
        } else {
            tracing::info!("running in no workspace mode");
            self.configs.no_workspace_mode();
        }

        let mut rename_provider: OneOf<bool, RenameOptions> = OneOf::Left(true);

        if params
            .capabilities
            .text_document
            .and_then(|cap| cap.rename)
            .and_then(|r| r.prepare_support)
            .unwrap_or_default()
        {
            rename_provider = OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            });
        }

        let response = InitializeResult {
            capabilities: ServerCapabilities {
                // todo(): We might prefer incremental sync at some later stage
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                workspace: workspace_capabilities,
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                rename_provider: Some(rename_provider),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),

                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        };

        // Index all configured workspaces once at startup. This populates the
        // in-memory metamodel pool that `workspace/symbol` queries against,
        // keeping per-request symbol lookups free of on-the-fly workspace
        // re-scans and re-parses.
        let workspace_paths: Vec<PathBuf> = self
            .configs
            .get_workspaces()
            .into_iter()
            .filter_map(|workspace| workspace.to_file_path().ok())
            .collect();
        for workspace_path in workspace_paths {
            self.state.parse_all_from_workspace(&workspace_path, None);
        }

        Box::pin(async move { Ok(response) })
    }

    pub(super) fn shutdown(
        &mut self,
        _params: (),
    ) -> BoxFuture<'static, Result<(), ResponseError>> {
        info!("Received shutdown request");
        self.shutdown_received = true;
        Box::pin(async move { Ok(()) })
    }

    pub(super) fn hover(
        &self,
        param: HoverParams,
    ) -> BoxFuture<'static, Result<Option<Hover>, ResponseError>> {
        let uri = param.text_document_position_params.text_document.uri;
        let pos = param.text_document_position_params.position;

        let hover = self.state.hover(&uri, pos);

        Box::pin(async move { Ok(hover) })
    }

    pub(super) fn completion(
        &mut self,
        params: CompletionParams,
    ) -> BoxFuture<'static, Result<Option<CompletionResponse>, ResponseError>> {
        let uri = params.text_document_position.text_document.uri;

        // All keywords in the language
        let keywords = vec![
            "syntax", "package", "option", "import", "service", "rpc", "returns", "message",
            "enum", "oneof", "repeated", "reserved", "to",
        ];

        // Build completion item from builtins as fields
        let mut completions: Vec<CompletionItem> = docs::BUILTIN
            .iter()
            .map(|(k, v)| {
                (
                    k,
                    MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: v.to_string(),
                    },
                )
            })
            .map(|(k, v)| CompletionItem {
                label: k.to_string(),
                kind: Some(CompletionItemKind::FIELD),
                documentation: Some(Documentation::MarkupContent(v)),
                ..CompletionItem::default()
            })
            .collect();

        // Build completion item from keywords
        completions.extend(keywords.into_iter().map(|w| CompletionItem {
            label: w.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..CompletionItem::default()
        }));

        // Build completion item from the current document
        if let Some(document) = self.state.get_document(&uri) {
            let package_name = document.package_name();
            if package_name != "." {
                completions.extend(self.state.completion_items_for_package(package_name));
            }

            if let Some(ipath) = self.configs.get_include_paths(&uri) {
                for import in document.import_paths() {
                    if let Some(p) = ipath.iter().map(|p| p.join(&import)).find(|p| p.exists())
                        && let Ok(uri) = Url::from_file_path(p.clone())
                    {
                        completions.extend(self.state.completion_items_for_document(&uri));
                    }
                }
            }
        }
        Box::pin(async move { Ok(Some(CompletionResponse::Array(completions))) })
    }

    pub(super) fn prepare_rename(
        &mut self,
        params: TextDocumentPositionParams,
    ) -> BoxFuture<'static, Result<Option<PrepareRenameResponse>, ResponseError>> {
        let uri = params.text_document.uri;
        let pos = params.position;

        let Some(document) = self.state.get_document(&uri) else {
            error!(uri=%uri, "failed to get document");
            return Box::pin(async move { Ok(None) });
        };

        let response = document.can_rename(pos).map(PrepareRenameResponse::Range);

        Box::pin(async move { Ok(response) })
    }

    pub(super) fn rename(
        &mut self,
        params: RenameParams,
    ) -> BoxFuture<'static, Result<Option<WorkspaceEdit>, ResponseError>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;

        let ipath = self.configs.get_include_paths(&uri).unwrap_or_default();

        // Resolve the symbol under the cursor directly from the metamodel,
        // using its position (like hover / go-to-definition). This handles both
        // declaration sites and reference sites (pivoting to the referenced
        // declaration) without any string-based identifier reconstruction.
        let Some(target_fqn) = self.state.resolve_target_fqn(&uri, pos) else {
            error!(uri=%uri, "failed to resolve target fqn for rename");
            return Box::pin(async move { Ok(None) });
        };
        let Some((decl_uri, decl_pos)) = self.state.declaration_for_fqn(&target_fqn) else {
            error!(fqn=%target_fqn, "failed to locate declaration for rename");
            return Box::pin(async move { Ok(None) });
        };

        // The rpc/request/response chain rename is opt-in via the workspace's
        // `[config.rename]` settings; without a config it stays off.
        let chain_rpc_request_response = self
            .configs
            .get_config_for_uri(&uri)
            .is_some_and(|c| c.config.rename.chain_rpc_request_response);

        let ops = self.state.compute_rename_ops(
            &decl_uri,
            decl_pos,
            &new_name,
            &ipath,
            chain_rpc_request_response,
        );
        let Some(all_edits) = self.state.apply_rename_ops(&ops) else {
            error!(uri=%decl_uri, "failed to apply primary rename");
            return Box::pin(async move { Ok(None) });
        };

        let response = if all_edits.is_empty() {
            None
        } else {
            Some(WorkspaceEdit {
                changes: Some(all_edits),
                ..Default::default()
            })
        };

        Box::pin(async move { Ok(response) })
    }

    pub(super) fn references(
        &mut self,
        param: ReferenceParams,
    ) -> BoxFuture<'static, Result<Option<Vec<Location>>, ResponseError>> {
        let uri = param.text_document_position.text_document.uri;
        let pos = param.text_document_position.position;

        // The workspace is already fully indexed once at startup (see the
        // `initialize` handler), so cross-file reference resolution operates on
        // the cached metamodel pool without any per-request re-scan.
        let Some(target_fqn) = self.state.resolve_target_fqn(&uri, pos) else {
            error!(uri=%uri, "failed to resolve target fqn");
            return Box::pin(async move { Ok(None) });
        };

        let refs = self.state.references_for_fqn(&target_fqn);

        Box::pin(async move {
            if refs.is_empty() {
                Ok(None)
            } else {
                Ok(Some(refs))
            }
        })
    }

    pub(super) fn definition(
        &mut self,
        param: GotoDefinitionParams,
    ) -> BoxFuture<'static, Result<Option<GotoDefinitionResponse>, ResponseError>> {
        let uri = param.text_document_position_params.text_document.uri;
        let pos = param.text_document_position_params.position;

        let Some(document) = self.state.get_document(&uri) else {
            error!(uri=%uri, "failed to get document");
            return Box::pin(async move { Ok(None) });
        };

        let jump = document.get_jumpable_at_position(pos);
        let current_package_name = document.package_name();

        let Some(jump) = jump else {
            error!(uri=%uri, "failed to get jump identifier");
            return Box::pin(async move { Ok(None) });
        };

        let ipath = self.configs.get_include_paths(&uri).unwrap_or_default();
        let locations = self
            .state
            .definition(&ipath, current_package_name.as_ref(), jump);

        let response = match locations.len() {
            0 => None,
            1 => Some(GotoDefinitionResponse::Scalar(locations[0].clone())),
            2.. => Some(GotoDefinitionResponse::Array(locations)),
        };

        Box::pin(async move { Ok(response) })
    }

    pub(super) fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> BoxFuture<'static, Result<Option<DocumentSymbolResponse>, ResponseError>> {
        let uri = params.text_document.uri;

        let Some(document) = self.state.get_document(&uri) else {
            error!(uri=%uri, "failed to get document");
            return Box::pin(async move { Ok(None) });
        };

        let symbols = document.document_symbols();
        let response = DocumentSymbolResponse::Nested(symbols);

        Box::pin(async move { Ok(Some(response)) })
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn workspace_symbol(
        &mut self,
        params: WorkspaceSymbolParams,
    ) -> BoxFuture<'static, Result<Option<WorkspaceSymbolResponse>, ResponseError>> {
        let query = params.query.to_lowercase();

        // Workspaces are indexed once at startup; symbol lookups are now a
        // pure in-memory substring scan over the cached metamodel pool.
        let symbols = self.state.find_workspace_symbols(&query);

        Box::pin(async move {
            if symbols.is_empty() {
                Ok(None)
            } else {
                Ok(Some(WorkspaceSymbolResponse::Nested(symbols)))
            }
        })
    }

    pub(super) fn formatting(
        &mut self,
        params: DocumentFormattingParams,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, ResponseError>> {
        let uri = params.text_document.uri;
        let content = self.state.get_content(&uri);

        let response = self
            .configs
            .get_formatter_for_uri(&uri)
            .and_then(|f| f.format_document(uri.path(), content.as_str()));

        Box::pin(async move { Ok(response) })
    }

    pub(super) fn range_formatting(
        &mut self,
        params: DocumentRangeFormattingParams,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, ResponseError>> {
        let uri = params.text_document.uri;
        let content = self.state.get_content(&uri);

        let response = self
            .configs
            .get_formatter_for_uri(&uri)
            .and_then(|f| f.format_document_range(&params.range, uri.path(), content.as_str()));

        Box::pin(async move { Ok(response) })
    }

    pub(super) fn did_save(
        &mut self,
        params: DidSaveTextDocumentParams,
    ) -> ControlFlow<async_lsp::Result<()>> {
        let uri = params.text_document.uri;
        let content = self.state.get_content(&uri);

        let Some(ipath) = self.configs.get_include_paths(&uri) else {
            return ControlFlow::Continue(());
        };

        let Some(pconf) = self.configs.get_config_for_uri(&uri) else {
            return ControlFlow::Continue(());
        };

        if let Some(diagnostics) =
            self.state
                .upsert_file(&uri, &content, &ipath, 8, &pconf.config, true)
            && let Err(e) = self.client.publish_diagnostics(diagnostics)
        {
            error!(error=%e, "failed to publish diagnostics");
        }
        ControlFlow::Continue(())
    }

    pub(super) fn did_open(
        &mut self,
        params: DidOpenTextDocumentParams,
    ) -> ControlFlow<async_lsp::Result<()>> {
        let uri = params.text_document.uri;
        let content = params.text_document.text;

        let Some(ipath) = self.configs.get_include_paths(&uri) else {
            return ControlFlow::Continue(());
        };

        let Some(pconf) = self.configs.get_config_for_uri(&uri) else {
            return ControlFlow::Continue(());
        };

        if let Some(diagnostics) =
            self.state
                .upsert_file(&uri, &content, &ipath, 8, &pconf.config, true)
            && let Err(e) = self.client.publish_diagnostics(diagnostics)
        {
            error!(error=%e, "failed to publish diagnostics");
        }
        ControlFlow::Continue(())
    }

    pub(super) fn did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> ControlFlow<async_lsp::Result<()>> {
        let uri = params.text_document.uri;
        let content = params.content_changes[0].text.clone();

        let Some(ipath) = self.configs.get_include_paths(&uri) else {
            return ControlFlow::Continue(());
        };

        let Some(pconf) = self.configs.get_config_for_uri(&uri) else {
            return ControlFlow::Continue(());
        };

        if let Some(diagnostics) =
            self.state
                .upsert_file(&uri, &content, &ipath, 8, &pconf.config, false)
            && let Err(e) = self.client.publish_diagnostics(diagnostics)
        {
            error!(error=%e, "failed to publish diagnostics");
        }
        ControlFlow::Continue(())
    }

    pub(super) fn did_create_files(
        &mut self,
        params: CreateFilesParams,
    ) -> ControlFlow<async_lsp::Result<()>> {
        for file in params.files {
            if let Ok(uri) = Url::from_file_path(&file.uri) {
                // Safety: The uri is always a file type
                let content = read_to_string(uri.to_file_path().unwrap()).unwrap_or_default();

                if let Some(ipath) = self.configs.get_include_paths(&uri) {
                    self.state.upsert_content(&uri, &content, &ipath, 2);
                }
            }
        }
        ControlFlow::Continue(())
    }

    pub(super) fn did_rename_files(
        &mut self,
        params: RenameFilesParams,
    ) -> ControlFlow<async_lsp::Result<()>> {
        for file in params.files {
            let Ok(new_uri) = Url::from_file_path(&file.new_uri) else {
                error!(uri = file.new_uri, "failed to parse uri");
                continue;
            };

            let Ok(old_uri) = Url::from_file_path(&file.old_uri) else {
                error!(uri = file.old_uri, "failed to parse uri");
                continue;
            };

            self.state.rename_file(&new_uri, &old_uri);
        }
        ControlFlow::Continue(())
    }

    pub(super) fn did_delete_files(
        &mut self,
        params: DeleteFilesParams,
    ) -> ControlFlow<async_lsp::Result<()>> {
        for file in params.files {
            if let Ok(uri) = Url::from_file_path(&file.uri) {
                self.state.delete_file(&uri);
            } else {
                error!(uri = file.uri, "failed to parse uri");
            }
        }
        ControlFlow::Continue(())
    }

    /// Handles the `$/setTrace` notification to dynamically update log verbosity.
    #[allow(clippy::needless_pass_by_value)]
    pub(super) fn set_trace(&mut self, params: SetTraceParams) -> ControlFlow<Result<(), Error>> {
        log::update_level(&self.log_handle, params.value);

        ControlFlow::Continue(())
    }

    pub(super) fn exit(&mut self, _params: ()) -> ControlFlow<async_lsp::Result<()>> {
        if self.shutdown_received {
            info!("Received exit notification after shutdown, exiting with code 0");
            std::process::exit(0);
        } else {
            warn!("Received exit notification without shutdown, exiting with code 1");
            std::process::exit(1);
        }
    }
}

/// Parse `include_paths` from initialization options
fn parse_init_include_paths(init_options: &Value) -> Option<Vec<PathBuf>> {
    let mut result = vec![];
    let paths = init_options["include_paths"].as_array()?;

    for path_value in paths {
        if let Some(path) = path_value.as_str() {
            result.push(PathBuf::from(path));
        } else {
            warn!(
                "Invalid include path in initialization options: {:?}",
                path_value
            );
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_init_include_paths_array() {
        let init_options = json!({
            "include_paths": ["/path/to/protos", "relative/path"]
        });

        let result = parse_init_include_paths(&init_options).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], PathBuf::from("/path/to/protos"));
        assert_eq!(result[1], PathBuf::from("relative/path"));
    }

    #[test]
    fn test_parse_init_include_paths_missing() {
        let init_options = json!({
            "other_option": "value"
        });

        let result = parse_init_include_paths(&init_options);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_init_include_paths_invalid_format() {
        let init_options = json!({
            "include_paths": 123
        });

        let result = parse_init_include_paths(&init_options);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_init_include_paths_mixed_array() {
        let init_options = json!({
            "include_paths": ["/valid/path", 123, "another/valid/path"]
        });

        let result = parse_init_include_paths(&init_options).unwrap();
        assert_eq!(result.len(), 2); // Only valid strings should be included
        assert_eq!(result[0], PathBuf::from("/valid/path"));
        assert_eq!(result[1], PathBuf::from("another/valid/path"));
    }

    #[test]
    fn test_initialization_options_integration() {
        // Test what a real client would send
        let neovim_style_init_options = json!({
            "include_paths": [
                "/usr/local/include/protobuf",
                "vendor/protos",
                "../shared-protos"
            ]
        });

        let include_paths = parse_init_include_paths(&neovim_style_init_options).unwrap();

        assert_eq!(include_paths.len(), 3);
        assert_eq!(
            include_paths[0],
            PathBuf::from("/usr/local/include/protobuf")
        );
        assert_eq!(include_paths[1], PathBuf::from("vendor/protos"));
        assert_eq!(include_paths[2], PathBuf::from("../shared-protos"));
    }
}
