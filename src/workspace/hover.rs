use async_lsp::lsp_types::MarkedString;

use crate::{
    formatter::ProtoFormatter, state::ProtoLanguageState, utils::split_identifier_package,
};

impl<F: ProtoFormatter> ProtoLanguageState<F> {
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

    use crate::{formatter::clang::ClangFormatter, state::ProtoLanguageState};

    #[test]
    fn workspace_test_hover() {
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState<ClangFormatter> = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned());
        state.upsert_file(&b_uri, b.to_owned());
        state.upsert_file(&c_uri, c.to_owned());

        assert_yaml_snapshot!(state.hover("com.workspace", "Author"));
        assert_yaml_snapshot!(state.hover("com.workspace", "Author.Address"));
        assert_yaml_snapshot!(state.hover("com.workspace", "com.utility.Foobar.Baz"));
        assert_yaml_snapshot!(state.hover("com.utility", "Baz"));
    }
}
