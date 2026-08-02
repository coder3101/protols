use std::path::PathBuf;

use async_lsp::lsp_types::{Location, Position, Range, Url};

use crate::{
    model::{ElementKind, SpatialEntry},
    state::ProtoLanguageState,
};

impl ProtoLanguageState {
    /// Resolves the target definition location(s) for the symbol under
    /// `position`.
    ///
    /// The jump kind is inferred directly from the metamodel element at the
    /// cursor: an `import` statement jumps to the imported file, while any
    /// other symbol (or a type reference) is resolved to its declaration via
    /// the shared cross-file name resolution engine.
    pub fn definition(&self, uri: &Url, pos: Position, ipath: &[PathBuf]) -> Vec<Location> {
        let Some(document) = self.get_document(uri) else {
            return vec![];
        };
        let Some(SpatialEntry { element_id, .. }) = document.find_entry_at_position(pos) else {
            return vec![];
        };
        let Some(element) = document.elements.get(*element_id) else {
            return vec![];
        };

        if let ElementKind::Import { path } = &element.kind {
            let Some(p) = ipath.iter().map(|p| p.join(path)).find(|p| p.exists()) else {
                return vec![];
            };
            let Ok(uri) = Url::from_file_path(p) else {
                return vec![];
            };
            return vec![Location {
                uri,
                range: Range::default(), // just start of the file
            }];
        }

        let Some(fqn) = self.resolve_target_fqn(uri, pos) else {
            return vec![];
        };
        self.declarations_for_fqn(&fqn)
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Position, Url};
    use std::path::PathBuf;

    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;

    fn setup_workspace() -> (Vec<PathBuf>, Url, Url, Url, ProtoLanguageState) {
        let ipath = vec![PathBuf::from("src/state/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(
            &a_uri,
            include_str!("input/a.proto"),
            &ipath,
            2,
            &Config::default(),
            false,
        );
        state.upsert_file(
            &b_uri,
            include_str!("input/b.proto"),
            &ipath,
            2,
            &Config::default(),
            false,
        );
        state.upsert_file(
            &c_uri,
            include_str!("input/c.proto"),
            &ipath,
            2,
            &Config::default(),
            false,
        );

        (ipath, a_uri, b_uri, c_uri, state)
    }

    #[test]
    fn workspace_test_definition_identifiers() {
        let (_ipath, _a, _b, _c, state) = setup_workspace();

        assert_yaml_snapshot!(state.resolve_identifier_locations("com.workspace", "Author"));
        assert_yaml_snapshot!(
            state.resolve_identifier_locations("com.workspace", "Author.Address")
        );
        assert_yaml_snapshot!(
            state.resolve_identifier_locations("com.workspace", "com.utility.Foobar.Baz")
        );
        assert_yaml_snapshot!(state.resolve_identifier_locations("com.utility", "Baz"));
    }

    #[test]
    fn test_definition_position_based() {
        let (ipath, a_uri, b_uri, _c_uri, state) = setup_workspace();

        // Cursor on the `Author` message declaration name in b.proto.
        assert_yaml_snapshot!(state.definition(
            &b_uri,
            Position {
                line: 5,
                character: 10
            },
            &ipath
        ));
        // Cursor on the `Author` type reference inside a field in a.proto.
        assert_yaml_snapshot!(state.definition(
            &a_uri,
            Position {
                line: 11,
                character: 5
            },
            &ipath
        ));
        // Cursor on empty whitespace -> no definition.
        assert!(
            state
                .definition(
                    &a_uri,
                    Position {
                        line: 0,
                        character: 0
                    },
                    &ipath
                )
                .is_empty()
        );
    }

    #[test]
    fn test_definition_import_position_based() {
        let ipath = vec![std::env::current_dir().unwrap().join("src/state/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(
            &a_uri,
            include_str!("input/a.proto"),
            &ipath,
            2,
            &Config::default(),
            false,
        );

        // Cursor on the `import "c.proto"` statement -> jump to the file.
        let loc = state.definition(
            &a_uri,
            Position {
                line: 4,
                character: 10,
            },
            &ipath,
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
        assert_yaml_snapshot!(state.resolve_identifier_locations("com.workspace", "Library"));
        // Jump to an rpc method.
        assert_yaml_snapshot!(state.resolve_identifier_locations("com.workspace", "GetBook"));
        // Jump to a field inside a message.
        assert_yaml_snapshot!(
            state.resolve_identifier_locations("com.workspace", "GetBookResponse.title")
        );
        // Fully-qualified with a leading dot.
        assert_yaml_snapshot!(
            state.resolve_identifier_locations("com.workspace", ".com.workspace.GetBookRequest")
        );
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
        assert_yaml_snapshot!(state.resolve_identifier_locations("com.enums", "Color"));
        // Jump to an enum value.
        assert_yaml_snapshot!(state.resolve_identifier_locations("com.enums", "Color.RED"));
        // Unknown symbol resolves to nothing.
        assert!(
            state
                .resolve_identifier_locations("com.enums", "Nope")
                .is_empty()
        );
    }
}
