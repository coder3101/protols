use async_lsp::lsp_types::{Position, Range};

use crate::{
    model::{ElementKind, SpatialEntry, TypeReference},
    utils::is_position_inside_range,
};

use super::parser::ProtoDocument;

impl ProtoDocument {
    pub fn can_rename(&self, pos: Position) -> Option<Range> {
        let SpatialEntry { element_id, .. } = self.find_entry_at_position(pos)?;
        let element = self.elements.get(*element_id)?;
        if matches!(element.kind, ElementKind::Import { .. }) {
            return None;
        }
        if is_position_inside_range(pos, element.meta.selection_range) {
            return Some(element.meta.selection_range);
        }
        // Cursor rests on a type reference; return the precise segment range.
        let type_ref = element.type_reference_at(pos)?;
        type_ref_segment_range(type_ref, pos)
    }

    /// If the given position is on the rpc name of an rpc declaration, returns
    /// the rpc's name along with its declared request and response type texts.
    /// Used to drive the rpc/request/response chained rename.
    pub fn rpc_at_position(
        &self,
        pos: Position,
        _content: impl AsRef<[u8]>,
    ) -> Option<(String, String, String)> {
        let SpatialEntry { element_id, .. } = self.find_entry_at_position(pos)?;
        let element = self.elements.get(*element_id)?;
        let ElementKind::Rpc {
            request_type_ref,
            response_type_ref,
            ..
        } = &element.kind
        else {
            return None;
        };
        if !is_position_inside_range(pos, element.meta.selection_range) {
            return None;
        }
        Some((
            element.meta.name.clone(),
            request_type_ref.name.clone(),
            response_type_ref.name.clone(),
        ))
    }

    /// If the given position is on a message name, returns that name.
    pub fn message_name_at_position(
        &self,
        pos: Position,
        _content: impl AsRef<[u8]>,
    ) -> Option<String> {
        let SpatialEntry { element_id, .. } = self.find_entry_at_position(pos)?;
        let element = self.elements.get(*element_id)?;
        if !matches!(element.kind, ElementKind::Message { .. }) {
            return None;
        }
        if !is_position_inside_range(pos, element.meta.selection_range) {
            return None;
        }
        Some(element.meta.name.clone())
    }

    /// Returns the (request, response) type texts for every `rpc` element in
    /// this document. Used to verify that a request/response type is uniquely used
    /// by a single rpc before chain-renaming it.
    pub fn all_rpc_signatures(&self, _content: impl AsRef<[u8]>) -> Vec<(String, String)> {
        self.elements
            .iter()
            .filter_map(|element| match &element.kind {
                ElementKind::Rpc {
                    request_type_ref,
                    response_type_ref,
                    ..
                } => Some((
                    request_type_ref.name.clone(),
                    response_type_ref.name.clone(),
                )),
                _ => None,
            })
            .collect()
    }

}

/// Determines which dot-separated segment of a type reference the cursor rests
/// on and returns that segment's precise range, based on the cursor's character
/// offset within the reference's range.
fn type_ref_segment_range(type_ref: &TypeReference, position: Position) -> Option<Range> {
    if position.line != type_ref.range.start.line {
        return None;
    }
    let line = type_ref.range.start.line;
    let mut cursor = type_ref.range.start.character as usize;
    let position_char = position.character as usize;
    for segment in type_ref.name.split('.') {
        let seg_start = cursor;
        let seg_end = cursor + segment.len();
        if position_char >= seg_start && position_char < seg_end {
            return Some(Range {
                start: Position {
                    line,
                    character: u32::try_from(seg_start).ok()?,
                },
                end: Position {
                    line,
                    character: u32::try_from(seg_end).ok()?,
                },
            });
        }
        cursor = seg_end + 1; // skip the '.' separator
    }
    None
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Position, Url};
    use insta::assert_yaml_snapshot;

    use crate::document::parser::ProtoParser;
    use crate::utils::compile_test_query;

    #[test]
    fn test_can_rename() {
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let pos_rename = Position {
            line: 5,
            character: 9,
        };
        let pos_non_rename = Position {
            line: 2,
            character: 2,
        };
        let pos_inner_type = Position {
            line: 19,
            character: 11,
        };
        let pos_outer_type = Position {
            line: 19,
            character: 5,
        };

        let contents = include_str!("input/test_can_rename.proto");

        let parsed = ProtoParser::new().parse(uri, contents, &compile_test_query());
        assert!(parsed.is_some());

        let document = parsed.unwrap();
        assert_yaml_snapshot!(document.can_rename(pos_rename));
        assert_yaml_snapshot!(document.can_rename(pos_non_rename));
        assert_yaml_snapshot!(document.can_rename(pos_inner_type));
        assert_yaml_snapshot!(document.can_rename(pos_outer_type));
    }

    #[test]
    fn test_can_rename_service_and_rpc() {
        let uri: Url = "file://foo/bar/test.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new().parse(uri, contents, &compile_test_query());
        assert!(parsed.is_some());
        let document = parsed.unwrap();

        let pos_service = Position {
            line: 10,
            character: 10,
        };
        let pos_rpc = Position {
            line: 11,
            character: 9,
        };
        let pos_rpc_request_type = Position {
            line: 11,
            character: 17,
        };

        assert_yaml_snapshot!(document.can_rename(pos_service));
        assert_yaml_snapshot!(document.can_rename(pos_rpc));
        // Type references inside an RPC declaration are renameable from the
        // reference site; the LSP layer pivots to the declaration.
        assert_yaml_snapshot!(document.can_rename(pos_rpc_request_type));
    }

    #[test]
    fn test_rpc_at_position_and_signatures() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new()
            .parse(uri, contents, &compile_test_query())
            .unwrap();

        // `GetBook` rpc at line 11 chars 8..15
        let pos_rpc = Position {
            line: 11,
            character: 10,
        };
        assert_eq!(
            parsed.rpc_at_position(pos_rpc, contents),
            Some(("GetBook".to_owned(), "Empty".to_owned(), "Book".to_owned(),)),
        );

        // Cursor on a non-rpc identifier should return None.
        let pos_service = Position {
            line: 10,
            character: 10,
        };
        assert_eq!(parsed.rpc_at_position(pos_service, contents), None);

        // all_rpc_signatures should pick up both rpcs in the file.
        let sigs = parsed.all_rpc_signatures(contents);
        assert_eq!(
            sigs,
            vec![
                ("Empty".to_owned(), "Book".to_owned()),
                ("Empty".to_owned(), "Book".to_owned()),
            ]
        );
    }

    #[test]
    fn test_message_name_at_position() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_service.proto");
        let parsed = ProtoParser::new()
            .parse(uri, contents, &compile_test_query())
            .unwrap();

        // `Book` declaration at line 6 chars 8..12
        let pos = Position {
            line: 6,
            character: 9,
        };
        assert_eq!(
            parsed.message_name_at_position(pos, contents),
            Some("Book".to_owned())
        );

        // RPC name shouldn't match.
        let pos_rpc = Position {
            line: 11,
            character: 10,
        };
        assert_eq!(parsed.message_name_at_position(pos_rpc, contents), None);
    }

    #[test]
    fn test_can_rename_field_and_enum_value() {
        let uri: Url = "file://foo/bar.proto".parse().unwrap();
        let contents = include_str!("input/test_rename_field.proto");
        let parsed = ProtoParser::new()
            .parse(uri, contents, &compile_test_query())
            .unwrap();

        // Cursor on a type identifier inside a field (`Author` at line 15
        // chars 4..10) is a reference site — already supported.
        let pos_type_ref = Position {
            line: 15,
            character: 6,
        };
        // Cursor on the field name should now be renameable.
        let pos_plain_field = Position {
            line: 14,
            character: 12,
        };
        // Cursor on the int_lit `1` is not a renameable identifier.
        let pos_field_number = Position {
            line: 14,
            character: 19,
        };

        assert_yaml_snapshot!(parsed.can_rename(pos_type_ref));
        assert_yaml_snapshot!(parsed.can_rename(pos_plain_field));
        assert_yaml_snapshot!(parsed.can_rename(pos_field_number));
    }
}
