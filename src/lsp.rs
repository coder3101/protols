use std::ops::ControlFlow;
use std::{collections::HashMap, fs::read_to_string};
use tracing::{error, info};

use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    CreateFilesParams, DeleteFilesParams, DidChangeConfigurationParams,
    DidChangeTextDocumentParams, DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
    DocumentRangeFormattingParams, DocumentSymbolParams, DocumentSymbolResponse, Documentation,
    FileOperationFilter, FileOperationPattern, FileOperationPatternKind,
    FileOperationRegistrationOptions, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializeParams, InitializeResult,
    Location, MarkupContent, MarkupKind, OneOf, PrepareRenameResponse, ReferenceParams,
    RenameFilesParams, RenameOptions, RenameParams, ServerCapabilities, ServerInfo,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url,
    WorkspaceEdit, WorkspaceFileOperationsServerCapabilities, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};
use async_lsp::{LanguageClient, LanguageServer, ResponseError};
use futures::future::BoxFuture;

use crate::docs;
use crate::formatter::ProtoFormatter;
use crate::server::ProtoLanguageServer;

impl LanguageServer for ProtoLanguageServer {
    type Error = ResponseError;
    type NotifyResult = ControlFlow<async_lsp::Result<()>>;

    fn initialize(
        &mut self,
        params: InitializeParams,
    ) -> BoxFuture<'static, Result<InitializeResult, Self::Error>> {
        let (cname, version) = params
            .client_info
            .as_ref()
            .map(|c| (c.name.as_str(), c.version.as_deref()))
            .unwrap_or(("<unknown>", None));

        let cversion = version.unwrap_or("<unknown>");

        info!("Connected with client {cname} {cversion}");

        let file_operation_filers = vec![FileOperationFilter {
            scheme: Some(String::from("file")),
            pattern: FileOperationPattern {
                glob: String::from("**/*.{proto}"),
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
            })
        } else {
            tracing::info!("running in no workspace mode");
            self.configs.no_workspace_mode()
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
                work_done_progress_options: Default::default(),
            })
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

        Box::pin(async move { Ok(response) })
    }

    fn hover(
        &mut self,
        param: HoverParams,
    ) -> BoxFuture<'static, Result<Option<Hover>, Self::Error>> {
        let uri = param.text_document_position_params.text_document.uri;
        let pos = param.text_document_position_params.position;

        let Some(tree) = self.state.get_tree(&uri) else {
            error!(uri=%uri, "failed to get tree");
            return Box::pin(async move { Ok(None) });
        };

        let content = self.state.get_content(&uri);
        let hv = tree.get_hoverable_at_position(&pos, content.as_bytes());
        let current_package_name = tree.get_package_name(content.as_bytes()).unwrap_or(".");

        let Some(hv) = hv else {
            error!(uri=%uri, "failed to get hoverable identifier");
            return Box::pin(async move { Ok(None) });
        };

        let ipath = self.configs.get_include_paths(&uri).unwrap_or_default();
        let result = self.state.hover(&ipath, current_package_name.as_ref(), hv);

        Box::pin(async move {
            Ok(result.map(|r| Hover {
                range: None,
                contents: HoverContents::Markup(r),
            }))
        })
    }

    fn completion(
        &mut self,
        params: CompletionParams,
    ) -> BoxFuture<'static, Result<Option<CompletionResponse>, Self::Error>> {
        let uri = params.text_document_position.text_document.uri;

        // All keywords in the language
        let keywords = vec![
            "syntax", "package", "option", "import", "service", "rpc", "returns", "message",
            "enum", "oneof", "repeated", "reserved", "to",
        ];

        // Build completion item from builtins as fields
        let mut completions: Vec<CompletionItem> = docs::BUITIN
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

        // Build completion item from the current tree
        if let Some(tree) = self.state.get_tree(&uri) {
            let content = self.state.get_content(&uri);
            if let Some(package_name) = tree.get_package_name(content.as_bytes()) {
                completions.extend(self.state.completion_items(package_name));
            }
        }
        Box::pin(async move { Ok(Some(CompletionResponse::Array(completions))) })
    }

    fn prepare_rename(
        &mut self,
        params: TextDocumentPositionParams,
    ) -> BoxFuture<'static, Result<Option<PrepareRenameResponse>, Self::Error>> {
        let uri = params.text_document.uri;
        let pos = params.position;

        let Some(tree) = self.state.get_tree(&uri) else {
            error!(uri=%uri, "failed to get tree");
            return Box::pin(async move { Ok(None) });
        };

        let response = tree.can_rename(&pos).map(PrepareRenameResponse::Range);

        Box::pin(async move { Ok(response) })
    }

    fn rename(
        &mut self,
        params: RenameParams,
    ) -> BoxFuture<'static, Result<Option<WorkspaceEdit>, Self::Error>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let new_name = params.new_name;

        let Some(tree) = self.state.get_tree(&uri) else {
            error!(uri=%uri, "failed to get tree");
            return Box::pin(async move { Ok(None) });
        };

        let content = self.state.get_content(&uri);

        let current_package = tree.get_package_name(content.as_bytes()).unwrap_or(".");

        let Some((edit, otext, ntext)) = tree.rename_tree(&pos, &new_name, content.as_bytes())
        else {
            error!(uri=%uri, "failed to rename in a tree");
            return Box::pin(async move { Ok(None) });
        };

        let Some(workspace) = self.configs.get_workspace_for_uri(&uri) else {
            error!(uri=%uri, "failed to get workspace");
            return Box::pin(async move { Ok(None) });
        };

        let work_done_token = params.work_done_progress_params.work_done_token;
        let progress_sender = work_done_token.map(|token| self.with_report_progress(token));

        let mut h = HashMap::new();
        h.extend(self.state.rename_fields(
            current_package,
            &otext,
            &ntext,
            workspace.to_file_path().unwrap(),
            progress_sender,
        ));

        h.entry(tree.uri).or_insert(edit.clone()).extend(edit);

        let response = Some(WorkspaceEdit {
            changes: Some(h),
            ..Default::default()
        });

        Box::pin(async move { Ok(response) })
    }

    fn references(
        &mut self,
        param: ReferenceParams,
    ) -> BoxFuture<'static, Result<Option<Vec<Location>>, ResponseError>> {
        let uri = param.text_document_position.text_document.uri;
        let pos = param.text_document_position.position;
        let work_done_token = param.work_done_progress_params.work_done_token;

        let Some(tree) = self.state.get_tree(&uri) else {
            error!(uri=%uri, "failed to get tree");
            return Box::pin(async move { Ok(None) });
        };

        let content = self.state.get_content(&uri);

        let current_package = tree.get_package_name(content.as_bytes()).unwrap_or(".");

        let Some((mut refs, otext)) = tree.reference_tree(&pos, content.as_bytes()) else {
            error!(uri=%uri, "failed to find references in a tree");
            return Box::pin(async move { Ok(None) });
        };

        let Some(workspace) = self.configs.get_workspace_for_uri(&uri) else {
            error!(uri=%uri, "failed to get workspace");
            return Box::pin(async move { Ok(None) });
        };

        let progress_sender = work_done_token.map(|token| self.with_report_progress(token));

        if let Some(v) = self.state.reference_fields(
            current_package,
            &otext,
            workspace.to_file_path().unwrap(),
            progress_sender,
        ) {
            refs.extend(v);
        }

        Box::pin(async move {
            if refs.is_empty() {
                Ok(None)
            } else {
                Ok(Some(refs))
            }
        })
    }

    fn definition(
        &mut self,
        param: GotoDefinitionParams,
    ) -> BoxFuture<'static, Result<Option<GotoDefinitionResponse>, ResponseError>> {
        let uri = param.text_document_position_params.text_document.uri;
        let pos = param.text_document_position_params.position;

        let Some(tree) = self.state.get_tree(&uri) else {
            error!(uri=%uri, "failed to get tree");
            return Box::pin(async move { Ok(None) });
        };

        let content = self.state.get_content(&uri);
        let jump = tree.get_jumpable_at_position(&pos, content.as_bytes());
        let current_package_name = tree.get_package_name(content.as_bytes()).unwrap_or(".");

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

    fn document_symbol(
        &mut self,
        params: DocumentSymbolParams,
    ) -> BoxFuture<'static, Result<Option<DocumentSymbolResponse>, Self::Error>> {
        let uri = params.text_document.uri;

        let Some(tree) = self.state.get_tree(&uri) else {
            error!(uri=%uri, "failed to get tree");
            return Box::pin(async move { Ok(None) });
        };

        let content = self.state.get_content(&uri);
        let locations = tree.find_document_locations(content.as_bytes());
        let response = DocumentSymbolResponse::Nested(locations);

        Box::pin(async move { Ok(Some(response)) })
    }

    fn formatting(
        &mut self,
        params: DocumentFormattingParams,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, Self::Error>> {
        let uri = params.text_document.uri;
        let content = self.state.get_content(&uri);

        let response = self
            .configs
            .get_formatter_for_uri(&uri)
            .and_then(|f| f.format_document(uri.path(), content.as_str()));

        Box::pin(async move { Ok(response) })
    }

    fn range_formatting(
        &mut self,
        params: DocumentRangeFormattingParams,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, Self::Error>> {
        let uri = params.text_document.uri;
        let content = self.state.get_content(&uri);

        let response = self
            .configs
            .get_formatter_for_uri(&uri)
            .and_then(|f| f.format_document_range(&params.range, uri.path(), content.as_str()));

        Box::pin(async move { Ok(response) })
    }

    fn did_save(&mut self, params: DidSaveTextDocumentParams) -> Self::NotifyResult {
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
                .upsert_file(&uri, content, &ipath, 8, &pconf.config, true)
        {
            if let Err(e) = self.client.publish_diagnostics(diagnostics) {
                error!(error=%e, "failed to publish diagnostics")
            }
        }
        ControlFlow::Continue(())
    }

    fn did_close(&mut self, _params: DidCloseTextDocumentParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Self::NotifyResult {
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
                .upsert_file(&uri, content, &ipath, 8, &pconf.config, true)
        {
            if let Err(e) = self.client.publish_diagnostics(diagnostics) {
                error!(error=%e, "failed to publish diagnostics")
            }
        }
        ControlFlow::Continue(())
    }

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Self::NotifyResult {
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
                .upsert_file(&uri, content, &ipath, 8, &pconf.config, false)
        {
            if let Err(e) = self.client.publish_diagnostics(diagnostics) {
                error!(error=%e, "failed to publish diagnostics")
            }
        }
        ControlFlow::Continue(())
    }

    fn did_create_files(&mut self, params: CreateFilesParams) -> Self::NotifyResult {
        for file in params.files {
            if let Ok(uri) = Url::from_file_path(&file.uri) {
                // Safety: The uri is always a file type
                let content = read_to_string(uri.to_file_path().unwrap()).unwrap_or_default();

                if let Some(ipath) = self.configs.get_include_paths(&uri) {
                    self.state.upsert_content(&uri, content, &ipath, 2);
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn did_rename_files(&mut self, params: RenameFilesParams) -> Self::NotifyResult {
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

    fn did_delete_files(&mut self, params: DeleteFilesParams) -> Self::NotifyResult {
        for file in params.files {
            if let Ok(uri) = Url::from_file_path(&file.uri) {
                self.state.delete_file(&uri);
            } else {
                error!(uri = file.uri, "failed to parse uri");
            }
        }
        ControlFlow::Continue(())
    }

    // Required because of: https://github.com/coder3101/protols/issues/32
    fn did_change_configuration(&mut self, _: DidChangeConfigurationParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    // Required because when jumping to outside the workspace; this is triggered
    fn did_change_workspace_folders(
        &mut self,
        _: DidChangeWorkspaceFoldersParams,
    ) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }
}
