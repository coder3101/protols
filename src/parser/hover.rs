//! Spatial coordinate resolution and hover query layer for protobuf parsed
//! trees.
//!
//! This module implements high-performance geometric intersection algorithms
//! that map a precise cursor point (line and character) to memory-cached
//! metadata entities.

use async_lsp::lsp_types::Position;

use crate::model::SpatialEntry;

use super::ParsedTree;

impl ParsedTree {
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
        state.upsert_file(
            &uri,
            contents.to_string(),
            &ipath,
            3,
            &Config::default(),
            false,
        );

        let mut hover_results = Vec::new();

        if let Some(parsed_tree) = state.get_tree(&uri) {
            let mut tested_positions = std::collections::HashSet::new();

            for element in &parsed_tree.elements {
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
        state.upsert_file(&uri, String::new(), &ipath, 3, &Config::default(), false);

        let pos = Position {
            line: 0,
            character: 0,
        };
        assert!(state.hover(&uri, pos).is_none());

        let mut state_minimal = ProtoLanguageState::new();
        state_minimal.upsert_file(
            &uri,
            "syntax = \"proto3\";\npackage com.test;".to_string(),
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
}
