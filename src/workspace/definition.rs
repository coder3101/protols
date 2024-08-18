use async_lsp::lsp_types::Location;

use crate::{state::ProtoLanguageState, utils::split_identifier_package};

impl ProtoLanguageState {
    pub fn definition(&self, curr_package: &str, identifier: &str) -> Vec<Location> {
        let (mut package, identifier) = split_identifier_package(identifier);
        if package.is_empty() {
            package = curr_package;
        }
        self.get_trees_for_package(package)
            .into_iter()
            .fold(vec![], |mut v, tree| {
                v.extend(tree.definition(identifier, self.get_content(&tree.uri)));
                v
            })
    }
}

#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use crate::state::ProtoLanguageState;

    #[test]
    fn workspace_test_definition() {
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned());
        state.upsert_file(&b_uri, b.to_owned());
        state.upsert_file(&c_uri, c.to_owned());

        assert_yaml_snapshot!(state.definition("com.library", "Author"));
        assert_yaml_snapshot!(state.definition("com.library", "Author.Address"));
        assert_yaml_snapshot!(state.definition("com.library", "com.utility.Foobar.Baz"));
        assert_yaml_snapshot!(state.definition("com.utility", "Baz"));
    }
}
