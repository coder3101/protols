use async_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Url};

use crate::model::{ElementKind, ModelElement, SpatialEntry};
use crate::state::ProtoLanguageState;
use crate::utils::is_position_inside_range;

impl ProtoLanguageState {
    /// Dispatches a hover query, returning a formatted markdown tooltip and the
    /// highlighted text range.
    ///
    /// # Returns
    ///
    /// Returns `Some(Hover)` containing the unified markdown payload, or `None`
    /// if the token carries no hoverable metadata.
    pub fn hover(&self, uri: &Url, position: Position) -> Option<Hover> {
        let current_document = self.get_document(uri)?;

        let SpatialEntry { element_id, range } =
            current_document.find_entry_at_position(position).copied()?;
        let element = current_document.elements.get(element_id)?;

        let value = element.to_hover_markdown(position).or_else(|| {
            let scope = element.kind.fqn().unwrap_or(&current_document.package);
            element
                .inspect_nested_type_reference(position)
                .and_then(|type_name| {
                    self.resolve_reference(scope, type_name)
                        .into_iter()
                        .next()
                        .and_then(|target| {
                            target
                                .element
                                .to_hover_markdown(target.element.meta.selection_range.start)
                        })
                })
        })?;

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(range),
        })
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
        let ipath = vec![std::env::current_dir().unwrap().join("src/state/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();
        let x_uri = "file://input/inner/x.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");
        let x = include_str!("input/inner/x.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a, &ipath, 3, &Config::default(), false);
        state.upsert_file(&b_uri, b, &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c, &ipath, 2, &Config::default(), false);
        state.upsert_file(&x_uri, x, &ipath, 2, &Config::default(), false);

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

    #[test]
    fn test_hover_builtin_and_wellknown() {
        let ipath = vec![];
        let uri = "file:///hover.proto".parse().unwrap();
        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(
            &uri,
            concat!(
                "syntax = \"proto3\";\n",
                "package com.hover;\n",
                "message Book {\n",
                "  string title = 1;\n",
                "  google.protobuf.Any ctx = 2;\n",
                "}\n",
            ),
            &ipath,
            3,
            &Config::default(),
            false,
        );

        // Hover over the builtin `string` type.
        assert_yaml_snapshot!(state.hover(
            &uri,
            Position {
                line: 3,
                character: 3
            }
        ));
        // Hover over the field name `title`.
        assert_yaml_snapshot!(state.hover(
            &uri,
            Position {
                line: 3,
                character: 11
            }
        ));
        // Hover over the well-known `google.protobuf.Any` type.
        assert_yaml_snapshot!(state.hover(
            &uri,
            Position {
                line: 4,
                character: 3
            }
        ));
    }
}
