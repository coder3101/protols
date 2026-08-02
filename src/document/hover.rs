//! Spatial coordinate resolution and hover query layer for protobuf parsed
//! documents.
//!
//! This module implements high-performance geometric intersection algorithms
//! that map a precise cursor point (line and character) to memory-cached
//! metadata entities.

use async_lsp::lsp_types::Position;

use crate::{
    context::Jumpable,
    model::{ElementKind, SpatialEntry},
};

use super::parser::ProtoDocument;

impl ProtoDocument {
    /// Performs a search to locate the innermost spatial index block
    /// intersecting with the specified LSP [`Position`].
    ///
    /// # Returns
    ///
    /// Returns `Some(&SpatialEntry)` enclosing the intersected element
    /// identifier, or `None` if the cursor is hovering over empty whitespace or
    /// non-semantic tokens.
    pub fn find_entry_at_position(&self, position: Position) -> Option<&SpatialEntry> {
        if self.spatial_index.is_empty() {
            return None;
        }

        let max_idx = self
            .spatial_index
            .binary_search_by(|entry| entry.range.start.cmp(&position))
            .unwrap_or_else(|insert_idx| insert_idx.saturating_sub(1));

        self.spatial_index[..=max_idx]
            .iter()
            .rev()
            .find(|entry| entry.contains_position(position))
    }

    /// Resolves the symbol under `position` into a jumpable [`Jumpable`]
    /// (an import path or an identifier), backed by the spatial index.
    pub fn get_jumpable_at_position(&self, pos: Position) -> Option<Jumpable> {
        let SpatialEntry { element_id, .. } = self.find_entry_at_position(pos)?;
        let element = self.elements.get(*element_id)?;

        match &element.kind {
            ElementKind::Import { path } => Some(Jumpable::Import(path.clone())),
            _ => element
                .inspect_nested_type_reference(pos)
                .map(ToOwned::to_owned)
                .or_else(|| (!element.meta.name.is_empty()).then(|| element.meta.name.clone()))
                .map(Jumpable::Identifier),
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Hover, Position, Url};
    use insta::assert_yaml_snapshot;
    use serde::Serialize;

    use crate::config::Config;
    use crate::model::ElementKind;
    use crate::state::ProtoLanguageState;

    #[derive(Serialize, Debug)]
    struct HoverSnapshotEntry {
        target: String,
        hover: Hover,
    }

    fn run_hover_test(contents: &str, file_name: &str) -> Vec<HoverSnapshotEntry> {
        let uri = Url::parse(&format!("file:///virtual/{file_name}")).unwrap();
        let ipath = vec![];

        let mut state = ProtoLanguageState::new();
        state.upsert_file(&uri, contents, &ipath, 3, &Config::default(), false);

        let mut hover_results = Vec::new();

        if let Some(parsed_document) = state.get_document(&uri) {
            let mut tested_positions = std::collections::HashSet::new();

            for element in &parsed_document.elements {
                let mut targets = vec![(element.meta.selection_range.start, "definition")];

                match &element.kind {
                    ElementKind::Field { type_ref, .. }
                    | ElementKind::OneofField { type_ref, .. } => {
                        targets.push((type_ref.range.start, "type_use"));
                    }
                    ElementKind::MapField {
                        key_type_ref,
                        value_type_ref,
                        ..
                    } => {
                        targets.push((key_type_ref.range.start, "map_key_use"));
                        targets.push((value_type_ref.range.start, "map_value_use"));
                    }
                    ElementKind::Rpc {
                        request_type_ref,
                        response_type_ref,
                        ..
                    } => {
                        targets.push((request_type_ref.range.start, "rpc_req_use"));
                        targets.push((response_type_ref.range.start, "rpc_res_use"));
                    }
                    _ => {}
                }

                for (pos, context) in targets {
                    if !tested_positions.insert((pos.line, pos.character)) {
                        continue;
                    }

                    if let Some(hover_result) = state.hover(&uri, pos) {
                        let display_name = Some(element.meta.name.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(match &element.kind {
                                ElementKind::Import { path } => path.as_str(),
                                _ => "unknown_element",
                            });

                        hover_results.push(HoverSnapshotEntry {
                            target: format!("{display_name} [{context}]"),
                            hover: hover_result,
                        });
                    }
                }
            }
        }

        hover_results
    }

    #[test]
    fn test_proto2_hover() {
        let contents = include_str!("input/syntax_variants/test_proto2.proto");
        let results = run_hover_test(contents, "test_proto2.proto");
        assert_yaml_snapshot!("test_proto2_hover", results);
    }

    #[test]
    fn test_proto3_hover() {
        let contents = include_str!("input/syntax_variants/test_proto3.proto");
        let results = run_hover_test(contents, "test_proto3.proto");
        assert_yaml_snapshot!("test_proto3_hover", results);
    }

    #[test]
    fn test_editions_hover() {
        let contents = include_str!("input/syntax_variants/test_editions.proto");
        let results = run_hover_test(contents, "test_editions.proto");
        assert_yaml_snapshot!("test_editions_hover", results);
    }

    #[test]
    fn test_package_duplicate_hover() {
        let contents = include_str!("input/syntax_variants/test_package_duplicate.proto");
        let results = run_hover_test(contents, "test_package_duplicate.proto");
        assert_yaml_snapshot!("test_package_duplicate_hover", results);
    }

    #[test]
    fn test_hover_on_empty_and_minimal_file_safety() {
        let uri = Url::parse("file:///virtual/empty_test.proto").unwrap();
        let ipath = vec![];

        let mut state = ProtoLanguageState::new();
        state.upsert_file(&uri, "", &ipath, 3, &Config::default(), false);

        let pos = Position {
            line: 0,
            character: 0,
        };
        assert!(state.hover(&uri, pos).is_none());

        let mut state_minimal = ProtoLanguageState::new();
        state_minimal.upsert_file(
            &uri,
            "syntax = \"proto3\";\npackage com.test;",
            &ipath,
            3,
            &Config::default(),
            false,
        );

        let pos_mid = Position {
            line: 1,
            character: 5,
        };
        assert!(state_minimal.hover(&uri, pos_mid).is_none());
    }

    #[test]
    fn test_jumpable_at_position() {
        use crate::context::Jumpable;
        use crate::document::parser::ProtoParser;
        use crate::utils::compile_test_query;

        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let contents = include_str!("input/test_goto_definition.proto");
        let parsed = ProtoParser::new().parse(uri, contents, &compile_test_query());
        assert!(parsed.is_some());
        let document = parsed.unwrap();

        // Cursor on the nested `Author` type reference of a field -> identifier.
        assert_eq!(
            document.get_jumpable_at_position(Position::new(10, 5)),
            Some(Jumpable::Identifier("Author".to_owned()))
        );
        // Cursor on the `Author` message definition name -> identifier.
        assert_eq!(
            document.get_jumpable_at_position(Position::new(5, 15)),
            Some(Jumpable::Identifier("Author".to_owned()))
        );
        // Cursor on empty whitespace -> no jumpable.
        assert_eq!(document.get_jumpable_at_position(Position::new(0, 0)), None);
    }

    #[test]
    fn test_jumpable_import_at_position() {
        use crate::context::Jumpable;
        use crate::document::parser::ProtoParser;
        use crate::utils::compile_test_query;

        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let contents = "syntax = \"proto3\";\npackage com.test;\nimport \"dep.proto\";\n";
        let parsed = ProtoParser::new().parse(uri, contents, &compile_test_query());
        assert!(parsed.is_some());
        let document = parsed.unwrap();

        // Cursor anywhere on the import statement -> Import path.
        assert_eq!(
            document.get_jumpable_at_position(Position::new(2, 8)),
            Some(Jumpable::Import("dep.proto".to_owned()))
        );
        // Cursor on the package name -> no jumpable (package is not a spatial element).
        assert_eq!(document.get_jumpable_at_position(Position::new(1, 14)), None);
    }
}
