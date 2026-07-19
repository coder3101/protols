use async_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Url};

use crate::model::{ElementKind, ModelElement, SpatialEntry};
use crate::parser::ParsedTree;
use crate::state::ProtoLanguageState;
use crate::utils::{is_position_inside_range, split_identifier_package};

impl ProtoLanguageState {
    /// Dispatches a hover query, returning a formatted markdown tooltip and the
    /// highlighted text range.
    ///
    /// # Returns
    ///
    /// Returns `Some(Hover)` containing the unified markdown payload, or `None`
    /// if the token carries no hoverable metadata.
    pub fn hover(&self, uri: &Url, position: Position) -> Option<Hover> {
        let current_tree = self.get_tree(uri)?;

        let SpatialEntry { element_id, range } =
            current_tree.find_entry_at_position(position).copied()?;
        let element = current_tree.elements.get(element_id)?;

        let value = element.to_hover_markdown(position).or_else(|| {
            element
                .inspect_nested_type_reference(position)
                .and_then(|type_name| self.resolve_package_bound_type(&current_tree, type_name))
        })?;

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(range),
        })
    }

    /// Semi-dynamic, package-bound fallback mechanism designed to resolve type signatures
    /// within adjacent schemas matching the target namespace bounds.
    ///
    /// # Note
    ///
    /// <https://github.com/coder3101/protols/issues/130>
    ///
    /// This implementation relies on suffix-based matching and loose scope stitching
    /// as an interim Phase 1 strategy. It is scheduled to be completely replaced by a strict,
    /// index-backed Cross-File Name Resolution engine during Phase 3 of development.
    fn resolve_package_bound_type(
        &self,
        current_tree: &ParsedTree,
        type_name: &str,
    ) -> Option<String> {
        let (mut package, id_name) = split_identifier_package(type_name);
        let curr_package = &current_tree.package;

        if package.is_empty() {
            package = curr_package.as_str();
        }

        let mut candidate_trees = vec![];

        // Evaluate and resolve relative namespace cascading rules
        if curr_package != package {
            let root_segment = curr_package.split('.').next().unwrap_or_default();

            // Avoid generating redundant combinations if the signature is already fully-qualified
            if root_segment.is_empty() || !package.starts_with(root_segment) {
                let full_package = format!("{curr_package}.{package}");
                candidate_trees.append(&mut self.get_trees_for_package(&full_package));
            }
        }

        // Collect direct package trees mapped to the target namespace
        candidate_trees.append(&mut self.get_trees_for_package(package));

        // Evaluate FQN trailing intersections across compiled tree scopes
        candidate_trees
            .iter()
            .flat_map(|t| &t.elements)
            .find(|e| e.kind.fqn().is_some_and(|fqn| fqn.ends_with(id_name)))
            .and_then(|target| target.to_hover_markdown(target.meta.selection_range.start))
    }
}

impl ModelElement {
    /// Evaluates if the cursor is positioned strictly over an embedded type
    /// name token, returning its clean un-prefixed string identifier.
    ///
    /// This method maps geometric bounds of `TypeReference` entities (like
    /// field types, map keys, or RPC signatures), ensuring that cardinality
    /// tokens or stream modifiers do not affect type-level lookups.
    pub fn inspect_nested_type_reference(&self, position: Position) -> Option<&str> {
        match &self.kind {
            ElementKind::Field { type_ref, .. } | ElementKind::OneofField { type_ref, .. } => {
                is_position_inside_range(position, type_ref.range).then_some(type_ref.name.as_str())
            }
            ElementKind::MapField {
                key_type_ref,
                value_type_ref,
                ..
            } => is_position_inside_range(position, key_type_ref.range)
                .then_some(key_type_ref.name.as_str())
                .or_else(|| {
                    is_position_inside_range(position, value_type_ref.range)
                        .then_some(value_type_ref.name.as_str())
                }),
            ElementKind::Rpc {
                request_type_ref,
                response_type_ref,
                ..
            } => is_position_inside_range(position, request_type_ref.range)
                .then_some(request_type_ref.name.as_str())
                .or_else(|| {
                    is_position_inside_range(position, response_type_ref.range)
                        .then_some(response_type_ref.name.as_str())
                }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Position;
    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;
    #[test]
    fn workspace_test_hover() {
        let ipath = vec![std::env::current_dir().unwrap().join("src/workspace/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();
        let x_uri = "file://input/inner/x.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");
        let x = include_str!("input/inner/x.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 3, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&x_uri, x.to_owned(), &ipath, 2, &Config::default(), false);

        assert_yaml_snapshot!(state.hover(
            &a_uri,
            Position {
                line: 15,
                character: 10
            }
        ));
        assert_yaml_snapshot!(state.hover(
            &a_uri,
            Position {
                line: 11,
                character: 6
            }
        ));
        assert_yaml_snapshot!(state.hover(
            &b_uri,
            Position {
                line: 10,
                character: 7
            }
        ));
        assert_yaml_snapshot!(state.hover(
            &a_uri,
            Position {
                line: 12,
                character: 14
            }
        ));
        assert_yaml_snapshot!(state.hover(
            &a_uri,
            Position {
                line: 13,
                character: 16
            }
        ));
        assert_yaml_snapshot!(state.hover(
            &c_uri,
            Position {
                line: 12,
                character: 5
            }
        ));
        assert_yaml_snapshot!(state.hover(
            &a_uri,
            Position {
                line: 14,
                character: 10
            }
        ));
        assert_yaml_snapshot!(state.hover(
            &x_uri,
            Position {
                line: 9,
                character: 18
            }
        ));
        // relative path hover
        assert_yaml_snapshot!(state.hover(
            &x_uri,
            Position {
                line: 10,
                character: 4
            }
        ));
    }
}
