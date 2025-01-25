use std::path::PathBuf;

use async_lsp::lsp_types::{Location, Range, Url};

use crate::{
    context::jumpable::Jumpable, state::ProtoLanguageState, utils::split_identifier_package,
};

impl ProtoLanguageState {
    pub fn definition(
        &self,
        ipath: &[PathBuf],
        curr_package: &str,
        jump: Jumpable,
    ) -> Vec<Location> {
        match jump {
            Jumpable::Import(path) => {
                let Some(p) = ipath.iter().map(|p| p.join(&path)).find(|p| p.exists()) else {
                    return vec![];
                };

                let Ok(uri) = Url::from_file_path(p) else {
                    return vec![];
                };

                vec![Location {
                    uri,
                    range: Range::default(), // just start of the file
                }]
            }
            Jumpable::Identifier(identifier) => {
                let (mut package, identifier) = split_identifier_package(identifier.as_str());
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
    }
}

#[cfg(test)]
mod test {
    use crate::context::jumpable::Jumpable;
    use std::path::PathBuf;

    use insta::assert_yaml_snapshot;

    use crate::state::ProtoLanguageState;

    #[test]
    fn workspace_test_definition() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath);
        state.upsert_file(&b_uri, b.to_owned(), &ipath);
        state.upsert_file(&c_uri, c.to_owned(), &ipath);

        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.workspace",
            Jumpable::Identifier("Author".to_owned())
        ));
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.workspace",
            Jumpable::Identifier("Author.Address".to_owned())
        ));
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.workspace",
            Jumpable::Identifier("com.utility.Foobar.Baz".to_owned())
        ));
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.utility",
            Jumpable::Identifier("Baz".to_owned())
        ));

        let loc = state.definition(
            &vec![std::env::current_dir().unwrap().join(&ipath[0])],
            "com.workspace",
            Jumpable::Import("c.proto".to_owned()),
        );

        assert_eq!(loc.len(), 1);
        assert!(loc[0]
            .uri
            .to_file_path()
            .unwrap()
            .ends_with(ipath[0].join("c.proto")))
    }
}
