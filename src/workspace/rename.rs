use crate::nodekind::NodeKind;
use crate::utils::{split_identifier_package, ts_to_lsp_position};
use std::collections::HashMap;
use std::path::PathBuf;

use async_lsp::lsp_types::{Location, Range, TextEdit, Url};

use crate::state::ProtoLanguageState;
use async_lsp::lsp_types::ProgressParamsValue;
use std::sync::mpsc::Sender;

impl ProtoLanguageState {
    pub fn rename_fields(
        &mut self,
        current_package: &str,
        identifier: &str,
        new_text: &str,
        workspace: PathBuf,
        progress_sender: Option<Sender<ProgressParamsValue>>,
    ) -> HashMap<Url, Vec<TextEdit>> {
        self.parse_all_from_workspace(workspace, progress_sender);
        let (_, identifier) = split_identifier_package(identifier);
        self.get_trees()
            .into_iter()
            .fold(HashMap::new(), |mut h, tree| {
                let content = self.get_content(&tree.uri);
                let package = tree.get_package_name(content.as_ref()).unwrap_or(".");
                let mut old = identifier.to_string();
                let mut new = new_text.to_string();
                let mut v = vec![];

                // Global scope: Reference by only . or within global directly
                if current_package == "." {
                    if package == "." {
                        v.extend(tree.rename_field(&old, &new, content.as_str()));
                    }

                    old = format!(".{old}");
                    new = format!(".{new}");

                    v.extend(tree.rename_field(&old, &new, content.as_str()));

                    if !v.is_empty() {
                        h.insert(tree.uri.clone(), v);
                    }
                    return h;
                }

                let full_old = format!("{current_package}.{old}");
                let full_new = format!("{current_package}.{new}");
                let global_full_old = format!(".{current_package}.{old}");
                let global_full_new = format!(".{current_package}.{new}");

                // Current package: Reference by full or relative name or directly
                if current_package == package {
                    v.extend(tree.rename_field(&old, &new, content.as_str()));
                } else if current_package.starts_with(package) {
                    // Safety: prefix check already done
                    // get the relative part of the package
                    let packagepart = current_package
                        .strip_prefix(package)
                        .unwrap()
                        .trim_start_matches('.');
                    let relative_old = format!("{packagepart}.{old}");
                    let relative_new = format!("{packagepart}.{new}");
                    v.extend(tree.rename_field(&relative_old, &relative_new, content.as_str()));
                }

                // Otherwise, full reference
                v.extend(tree.rename_field(&full_old, &full_new, content.as_str()));
                v.extend(tree.rename_field(&global_full_old, &global_full_new, content.as_str()));

                if !v.is_empty() {
                    h.insert(tree.uri.clone(), v);
                }
                h
            })
    }

    pub fn reference_fields(
        &mut self,
        current_package: &str,
        identifier: &str,
        workspace: PathBuf,
        progress_sender: Option<Sender<ProgressParamsValue>>,
    ) -> Option<Vec<Location>> {
        self.parse_all_from_workspace(workspace, progress_sender);
        let (_, identifier) = split_identifier_package(identifier);
        let r = self
            .get_trees()
            .into_iter()
            .fold(Vec::<Location>::new(), |mut v, tree| {
                let content = self.get_content(&tree.uri);
                let package = tree.get_package_name(content.as_ref()).unwrap_or(".");
                let mut ident = identifier.to_owned();
                // Global scope: Reference by only . or within global directly
                if current_package == "." {
                    if package == "." {
                        v.extend(tree.reference_field(&ident, content.as_str()));
                    }

                    ident = format!(".{ident}");
                    v.extend(tree.reference_field(&ident, content.as_str()));

                    return v;
                }

                let full_ident = format!("{current_package}.{ident}");
                let global_full_ident = format!(".{current_package}.{ident}");

                // Current package: Reference by full or relative name or directly
                if current_package == package {
                    v.extend(tree.reference_field(&ident, content.as_str()));
                } else if current_package.starts_with(package) {
                    // Safety: prefix check already done
                    // get the relative part of the package
                    let packagepart = current_package
                        .strip_prefix(package)
                        .unwrap()
                        .trim_start_matches('.');
                    let relative = format!("{packagepart}.{ident}");
                    v.extend(tree.reference_field(&relative, content.as_str()));
                }

                // Otherwise, full reference
                v.extend(tree.reference_field(&full_ident, content.as_str()));
                v.extend(tree.reference_field(&global_full_ident, content.as_str()));
                v
            });
        if r.is_empty() { None } else { Some(r) }
    }

    /// Find the declaration of an rpc by simple name across the workspace.
    /// Returns the first match. Used by the rpc/request/response chain rename
    /// to anchor the chain when the user invokes rename on a matching message.
    pub fn find_rpc_decl(&self, rpc_name: &str) -> Option<Location> {
        for tree in self.get_trees() {
            let content = self.get_content(&tree.uri);
            for node in tree.find_all_nodes(NodeKind::is_identifier) {
                let Some(parent) = node.parent() else {
                    continue;
                };
                if parent.kind() != NodeKind::RpcName.as_str() {
                    continue;
                }
                let Ok(text) = node.utf8_text(content.as_bytes()) else {
                    continue;
                };
                if text == rpc_name {
                    return Some(Location {
                        uri: tree.uri.clone(),
                        range: Range {
                            start: ts_to_lsp_position(&node.start_position()),
                            end: ts_to_lsp_position(&node.end_position()),
                        },
                    });
                }
            }
        }
        None
    }

    /// Count the number of rpcs in the workspace whose request or response
    /// type's trailing identifier segment matches `type_simple_name`. Used for
    /// the uniqueness check before chain-renaming a request/response message.
    pub fn count_rpc_uses_of_type(&self, type_simple_name: &str) -> usize {
        let mut count = 0;
        for tree in self.get_trees() {
            let content = self.get_content(&tree.uri);
            for (req, resp) in tree.all_rpc_signatures(content.as_bytes()) {
                if trailing_segment(&req) == type_simple_name
                    || trailing_segment(&resp) == type_simple_name
                {
                    count += 1;
                }
            }
        }
        count
    }
}

fn trailing_segment(qualified: &str) -> &str {
    qualified.rsplit('.').next().unwrap_or(qualified)
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;

    #[test]
    fn test_rename() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);

        assert_yaml_snapshot!(state.rename_fields(
            "com.workspace",
            "Author",
            "Writer",
            PathBuf::from("src/workspace/input"),
            None
        ));
        assert_yaml_snapshot!(state.rename_fields(
            "com.workspace",
            "Author.Address",
            "Author.Location",
            PathBuf::from("src/workspace/input"),
            None
        ));
        assert_yaml_snapshot!(state.rename_fields(
            "com.utility",
            "Foobar.Baz",
            "Foobar.Baaz",
            PathBuf::from("src/workspace/input"),
            None
        ));
    }

    #[test]
    fn test_reference() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);

        assert_yaml_snapshot!(state.reference_fields(
            "com.workspace",
            "Author",
            PathBuf::from("src/workspace/input"),
            None
        ));
        assert_yaml_snapshot!(state.reference_fields(
            "com.workspace",
            "Author.Address",
            PathBuf::from("src/workspace/input"),
            None
        ));
    }

    #[test]
    fn test_find_rpc_decl_and_count_uses() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let msg_uri = "file://input/messages.proto".parse().unwrap();
        let svc = include_str!("input/service.proto");
        let msg = include_str!("input/messages.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(
            &svc_uri,
            svc.to_owned(),
            &ipath,
            2,
            &Config::default(),
            false,
        );
        state.upsert_file(
            &msg_uri,
            msg.to_owned(),
            &ipath,
            2,
            &Config::default(),
            false,
        );

        // Lookup hits the rpc_name node in service.proto.
        let loc = state.find_rpc_decl("GetBook").expect("rpc not found");
        assert!(loc.uri.as_str().ends_with("service.proto"));
        assert_eq!(loc.range.start.line, 7);

        assert!(state.find_rpc_decl("DoesNotExist").is_none());

        // Convention-following types are uniquely used.
        assert_eq!(state.count_rpc_uses_of_type("GetBookRequest"), 1);
        assert_eq!(state.count_rpc_uses_of_type("GetBookResponse"), 1);
        // Shared types are seen twice.
        assert_eq!(state.count_rpc_uses_of_type("SharedReq"), 2);
        assert_eq!(state.count_rpc_uses_of_type("SharedResp"), 2);
        // Non-convention type still uniquely used as a response.
        assert_eq!(state.count_rpc_uses_of_type("Bar"), 1);
        // Unrelated names see no uses.
        assert_eq!(state.count_rpc_uses_of_type("Unrelated"), 0);
    }
}
