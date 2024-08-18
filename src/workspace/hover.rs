use async_lsp::lsp_types::MarkedString;

use crate::{state::ProtoLanguageState, utils::split_identifier_package};

impl ProtoLanguageState {
    pub fn hover(&self, curr_package: &str, identifier: &str) -> Vec<MarkedString> {
        let (mut package, identifier) = split_identifier_package(identifier);
        if package.is_empty() {
            package = curr_package;
        }

        self.get_trees_for_package(package)
            .into_iter()
            .fold(vec![], |mut v, tree| {
                v.extend(tree.hover(identifier, self.get_content(&tree.uri)));
                v
            })
    }
}

#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use crate::state::ProtoLanguageState;

    #[test]
    fn workspace_test_hover() {
        let a_uri = "file://input/workspace_test_hover/a.proto".parse().unwrap();
        let b_uri = "file://input/workspace_test_hover/b.proto".parse().unwrap();
        let c_uri = "file://input/workspace_test_hover/c.proto".parse().unwrap();

        let a = include_str!("input/workspace_test_hover/a.proto");
        let b = include_str!("input/workspace_test_hover/b.proto");
        let c = include_str!("input/workspace_test_hover/c.proto");

        let mut state = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned());
        state.upsert_file(&b_uri, b.to_owned());
        state.upsert_file(&c_uri, c.to_owned());

        assert_yaml_snapshot!(state.hover("com.library", "Author"));
        assert_yaml_snapshot!(state.hover("com.library", "Author.Address"));
        assert_yaml_snapshot!(state.hover("com.library", "com.utility.Foobar.Baz"));
        assert_yaml_snapshot!(state.hover("com.utility", "com.library.Baz"));
    }
}
