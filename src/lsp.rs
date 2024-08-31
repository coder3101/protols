use std::collections::HashMap;
use std::fs::read_to_string;
use std::ops::ControlFlow;
use std::sync::mpsc;
use std::thread;
use tracing::{error, info};

use async_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams, CompletionResponse,
    CreateFilesParams, DeleteFilesParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
    DocumentRangeFormattingParams, DocumentSymbolParams, DocumentSymbolResponse,
    FileOperationFilter, FileOperationPattern, FileOperationPatternKind,
    FileOperationRegistrationOptions, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, OneOf,
    PrepareRenameResponse, ProgressParams, RenameFilesParams, RenameOptions, RenameParams,
    ServerCapabilities, ServerInfo, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextEdit, Url, WorkspaceEdit, WorkspaceFileOperationsServerCapabilities,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};
use async_lsp::{LanguageClient, LanguageServer, ResponseError};
use futures::future::BoxFuture;

use crate::formatter::clang::ClangFormatter;
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

        let worktoken = params.work_done_progress_params.work_done_token;
        let (tx, rx) = mpsc::channel();
        let mut socket = self.client.clone();

        thread::spawn(move || {
            let Some(token) = worktoken else {
                return;
            };

            while let Ok(value) = rx.recv() {
                if let Err(e) = socket.progress(ProgressParams {
                    token: token.clone(),
                    value,
                }) {
                    error!(error=%e, "failed to report parse progress");
                }
            }
        });

        let file_registration_option = FileOperationRegistrationOptions {
            filters: file_operation_filers.clone(),
        };

        let mut workspace_capabilities = None;
        let mut formatter_provider = None;
        let mut formatter_range_provider = None;
        if let Some(folders) = params.workspace_folders {
            if let Ok(f) = ClangFormatter::new("clang-format", folders.first().unwrap().uri.path())
            {
                self.state.add_formatter(f);
                formatter_provider = Some(OneOf::Left(true));
                formatter_range_provider = Some(OneOf::Left(true));
                info!("Setting formatting client capability");
            }
            for workspace in folders {
                info!("Workspace folder: {workspace:?}");
                self.state.add_workspace_folder_async(workspace, tx.clone())
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
                document_formatting_provider: formatter_provider,
                document_range_formatting_provider: formatter_range_provider,

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
        let identifier = tree.get_actionable_node_text_at_position(&pos, content.as_bytes());
        let current_package_name = tree.get_package_name(content.as_bytes());

        let Some(identifier) = identifier else {
            error!(uri=%uri, "failed to get identifier");
            return Box::pin(async move { Ok(None) });
        };

        let Some(current_package_name) = current_package_name else {
            error!(uri=%uri, "failed to get package name");
            return Box::pin(async move { Ok(None) });
        };

        let comments = self
            .state
            .hover(current_package_name.as_ref(), identifier.as_ref());

        let response = match comments.len() {
            0 => None,
            1 => Some(Hover {
                contents: HoverContents::Scalar(comments[0].clone()),
                range: None,
            }),
            2.. => Some(Hover {
                contents: HoverContents::Array(comments),
                range: None,
            }),
        };

        Box::pin(async move { Ok(response) })
    }
    fn completion(
        &mut self,
        params: CompletionParams,
    ) -> BoxFuture<'static, Result<Option<CompletionResponse>, Self::Error>> {
        let uri = params.text_document_position.text_document.uri;

        let keywords = vec![
            "syntax", "package", "option", "import", "service", "rpc", "returns", "message",
            "enum", "oneof", "repeated", "reserved", "to",
        ];

        let mut completions: Vec<CompletionItem> = keywords
            .into_iter()
            .map(|w| CompletionItem {
                label: w.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..CompletionItem::default()
            })
            .collect();

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

        let Some(current_package) = tree.get_package_name(content.as_bytes()) else {
            error!(uri=%uri, "failed to get package name");
            return Box::pin(async move { Ok(None) });
        };

        let Some((edit, otext, ntext)) = tree.rename_tree(&pos, &new_name, content.as_bytes())
        else {
            error!(uri=%uri, "failed to rename in a tree");
            return Box::pin(async move { Ok(None) });
        };

        let mut h = HashMap::new();
        h.insert(tree.uri.clone(), edit);
        h.extend(self.state.rename_fields(current_package, &otext, &ntext));

        let response = Some(WorkspaceEdit {
            changes: Some(h),
            ..Default::default()
        });

        Box::pin(async move { Ok(response) })
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
        let identifier = tree.get_actionable_node_text_at_position(&pos, content.as_bytes());
        let current_package_name = tree.get_package_name(content.as_bytes());

        let Some(identifier) = identifier else {
            error!(uri=%uri, "failed to get identifier");
            return Box::pin(async move { Ok(None) });
        };

        let Some(current_package_name) = current_package_name else {
            error!(uri=%uri, "failed to get package name");
            return Box::pin(async move { Ok(None) });
        };

        let locations = self
            .state
            .definition(current_package_name.as_ref(), identifier.as_ref());

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
            .state
            .get_formatter()
            .and_then(|f| f.format_document(content.as_str()));

        Box::pin(async move { Ok(response) })
    }

    fn range_formatting(
        &mut self,
        params: DocumentRangeFormattingParams,
    ) -> BoxFuture<'static, Result<Option<Vec<TextEdit>>, Self::Error>> {
        let uri = params.text_document.uri;
        let content = self.state.get_content(&uri);

        let response = self
            .state
            .get_formatter()
            .and_then(|f| f.format_document_range(&params.range, content.as_str()));

        Box::pin(async move { Ok(response) })
    }

    fn did_save(&mut self, _: DidSaveTextDocumentParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn did_close(&mut self, _params: DidCloseTextDocumentParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let content = params.text_document.text;

        if let Some(diagnostics) = self.state.upsert_file(&uri, content) {
            if let Err(e) = self.client.publish_diagnostics(diagnostics) {
                error!(error=%e, "failed to publish diagnostics")
            }
        }
        ControlFlow::Continue(())
    }

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Self::NotifyResult {
        let uri = params.text_document.uri;
        let content = params.content_changes[0].text.clone();

        if let Some(diagnostics) = self.state.upsert_file(&uri, content) {
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
                self.state.upsert_content(&uri, content);
            } else {
                error!(uri=%file.uri, "failed parse uri");
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
}
