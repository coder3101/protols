use crate::utils::split_identifier_package;
use std::collections::HashMap;

use async_lsp::lsp_types::{TextEdit, Url};

use crate::state::ProtoLanguageState;

impl ProtoLanguageState {
    pub fn rename_fields(
        &self,
        current_package: &str,
        identifier: &str,
        new_text: &str,
    ) -> HashMap<Url, Vec<TextEdit>> {
        let (_, identifier) = split_identifier_package(identifier);
        self.get_trees()
            .into_iter()
            .fold(HashMap::new(), |mut h, tree| {
                let content = self.get_content(&tree.uri);
                let package = tree.get_package_name(content.as_ref()).unwrap_or_default();
                let mut old = identifier.to_string();
                let mut new = new_text.to_string();
                if current_package != package {
                    old = format!("{current_package}.{old}");
                    new = format!("{current_package}.{new}");
                }
                let v = tree.rename_field(&old, &new, content.as_str());
                if !v.is_empty() {
                    h.insert(tree.uri.clone(), v);
                }
                h
            })
    }
}

#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use crate::state::ProtoLanguageState;

    #[test]
    fn test_rename() {
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned());
        state.upsert_file(&b_uri, b.to_owned());
        state.upsert_file(&c_uri, c.to_owned());

        assert_yaml_snapshot!(state.rename_fields("com.workspace", "Author", "Writer"));
        assert_yaml_snapshot!(state.rename_fields(
            "com.workspace",
            "Author.Address",
            "Author.Location"
        ));
        assert_yaml_snapshot!(state.rename_fields("com.utility", "Foobar.Baz", "Foobar.Baaz"));
    }
}
