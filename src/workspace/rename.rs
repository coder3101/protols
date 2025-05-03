use crate::utils::split_identifier_package;
use std::collections::HashMap;
use std::path::PathBuf;

use async_lsp::lsp_types::{Location, TextEdit, Url};

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
                let mut old = identifier.to_owned();
                // Global scope: Reference by only . or within global directly
                if current_package == "." {
                    if package == "." {
                        v.extend(tree.reference_field(&old, content.as_str()));
                    }

                    old = format!(".{old}");
                    v.extend(tree.reference_field(&old, content.as_str()));

                    return v;
                }

                let full_old = format!("{current_package}.{old}");
                let global_full_old = format!(".{current_package}.{old}");

                // Current package: Reference by full or relative name or directly
                if current_package == package {
                    v.extend(tree.reference_field(&old, content.as_str()));
                }

                // Otherwise, full reference
                v.extend(tree.reference_field(&full_old, content.as_str()));
                v.extend(tree.reference_field(&global_full_old, content.as_str()));
                v
            });
        if r.is_empty() { None } else { Some(r) }
    }
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
}
