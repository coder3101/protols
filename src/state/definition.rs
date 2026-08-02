use std::path::PathBuf;

use async_lsp::lsp_types::{Location, Range, Url};

use crate::{context::Jumpable, state::ProtoLanguageState, utils::split_identifier_package};

impl ProtoLanguageState {
    /// Resolves the target definition location(s) for a jumpable token.
    ///
    /// The identifier half delegates to the shared cross-file name resolution
    /// engine (see [`ProtoLanguageState::resolve_reference`]), which matches
    /// against the flat metamodel registry by Fully Qualified Name instead of
    /// re-traversing the syntax document.
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
                let (package_part, id_name) = split_identifier_package(identifier.as_str());
                let scope = if package_part.is_empty() {
                    curr_package
                } else {
                    package_part
                };

                self.resolve_reference(scope, id_name)
                    .into_iter()
                    .map(|target| Location {
                        uri: target.uri,
                        range: target.element.meta.selection_range,
                    })
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::context::Jumpable;
    use async_lsp::lsp_types::Url;
    use std::path::PathBuf;

    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;
    #[test]
    fn workspace_test_definition() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a, &ipath, 2, &Config::default(), false);
        state.upsert_file(&b_uri, b, &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c, &ipath, 2, &Config::default(), false);

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
            &[std::env::current_dir().unwrap().join(&ipath[0])],
            "com.workspace",
            Jumpable::Import("c.proto".to_owned()),
        );

        assert_yaml_snapshot!(loc, {"[0].uri" => insta::dynamic_redaction(|c, _| {
            assert!(c.as_str().unwrap().ends_with("c.proto"));
            "file://<redacted>/c.proto".to_string()
        })});
    }

    #[test]
    fn workspace_test_definition_service_rpc_field() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let svc_uri: Url = "file://input/service.proto".parse().unwrap();
        let msg_uri: Url = "file://input/messages.proto".parse().unwrap();

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(
            &svc_uri,
            include_str!("input/service.proto"),
            &ipath,
            2,
            &Config::default(),
            false,
        );
        state.upsert_file(
            &msg_uri,
            include_str!("input/messages.proto"),
            &ipath,
            2,
            &Config::default(),
            false,
        );

        // Jump to a service declaration.
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.workspace",
            Jumpable::Identifier("Library".to_owned())
        ));
        // Jump to an rpc method.
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.workspace",
            Jumpable::Identifier("GetBook".to_owned())
        ));
        // Jump to a field inside a message.
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.workspace",
            Jumpable::Identifier("GetBookResponse.title".to_owned())
        ));
        // Fully-qualified with a leading dot.
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.workspace",
            Jumpable::Identifier(".com.workspace.GetBookRequest".to_owned())
        ));
    }

    #[test]
    fn workspace_test_definition_enum_and_nonexistent() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let uri: Url = "file://input/enums.proto".parse().unwrap();
        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(
            &uri,
            concat!(
                "syntax = \"proto3\";\n",
                "package com.enums;\n",
                "enum Color { RED = 0; GREEN = 1; }\n",
            ),
            &ipath,
            2,
            &Config::default(),
            false,
        );

        // Jump to the enum type.
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.enums",
            Jumpable::Identifier("Color".to_owned())
        ));
        // Jump to an enum value.
        assert_yaml_snapshot!(state.definition(
            &ipath,
            "com.enums",
            Jumpable::Identifier("Color.RED".to_owned())
        ));
        // Unknown symbol resolves to nothing.
        assert!(state
            .definition(&ipath, "com.enums", Jumpable::Identifier("Nope".to_owned()))
            .is_empty());
    }
}
