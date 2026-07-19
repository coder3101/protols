//! Metamodel mutation and construction layer for protobuf schemas.
//!
//! This module groups all behavioral implementations responsible for
//! populating, mutating, and configuring data structures inside the pure-memory
//! metamodel registry.
//!
//! It isolates state-changing code from static domain declarations and
//! presentation logic.

use std::str::{FromStr, Utf8Error};

use async_lsp::lsp_types::{Position, Range};
use tree_sitter::Node;

use super::types::{
    CardinalityKind, ElementKind, FieldCardinality, ParseCardinalityError, TypeReference,
};

use crate::utils::to_lsp_range;

impl ElementKind {
    /// In-place mutation handle to inject the calculated Fully Qualified Name
    /// (FQN) into container or terminal elements.
    #[inline]
    pub fn set_fqn(&mut self, value: String) {
        if let Self::Message { fqn, .. }
        | Self::Enum { fqn, .. }
        | Self::Service { fqn, .. }
        | Self::Oneof { fqn }
        | Self::EnumValue { fqn, .. }
        | Self::Field { fqn, .. }
        | Self::OneofField { fqn, .. }
        | Self::MapField { fqn, .. }
        | Self::Rpc { fqn, .. } = self
        {
            *fqn = value;
        }
    }

    /// In-place mutation handle to apply the deprecation flag if the element
    /// explicitly contains a deprecation option block.
    #[inline]
    pub const fn set_deprecated(&mut self, value: bool) {
        if let Self::Message { is_deprecated, .. }
        | Self::Field { is_deprecated, .. }
        | Self::MapField { is_deprecated, .. }
        | Self::OneofField { is_deprecated, .. }
        | Self::Enum { is_deprecated, .. }
        | Self::EnumValue { is_deprecated, .. }
        | Self::Service { is_deprecated, .. }
        | Self::Rpc { is_deprecated, .. } = self
        {
            *is_deprecated = value;
        }
    }
}

impl TypeReference {
    /// Allocates an empty `TypeReference` container pre-allocated to the
    /// requested capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            name: String::with_capacity(capacity),
            range: Range::default(),
        }
    }

    /// Populates the type identifier reference string and its precise LSP
    /// boundaries directly from a raw Tree-sitter [`Node`].
    #[inline]
    pub fn fill(&mut self, node: Node, source: &[u8]) {
        let range = to_lsp_range(node);

        let trace_utf8_error = |e: &Utf8Error| {
            tracing::warn!(
                "Soft validation triggered: failed to extract valid UTF-8 text while parsing node {:?} at range {:?}. Details: {:?}",
                node,
                range,
                e
            );
        };
        if let Ok(raw_text) = node.utf8_text(source).inspect_err(trace_utf8_error) {
            self.name.clear();
            self.name.push_str(raw_text);
        }
        self.range = range;
    }
}

impl FromStr for CardinalityKind {
    type Err = ParseCardinalityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "required" => Ok(Self::Required),
            "optional" => Ok(Self::Optional),
            "repeated" => Ok(Self::Repeated),
            _ => Err(ParseCardinalityError),
        }
    }
}

impl FieldCardinality {
    /// Syntactically extracts the explicit field cardinality label and
    /// calculates its precise geometric text boundaries from the raw source
    /// code text block of a field.
    ///
    /// It isolates the presence strategy descriptor (`required`, `optional`, or
    /// `repeated`) from the field's datatype token, building a targeted range
    /// matching the word length.
    pub fn from_block_text(block_text: &str, block_range: Range) -> Option<Self> {
        let full_text = block_text.trim_start();

        let keyword = ["required", "optional", "repeated"]
            .iter()
            .copied()
            .find(|c| full_text.starts_with(c))?;

        let kind: CardinalityKind = keyword.parse().ok()?;

        // SAFETY: The `keyword` variable is guaranteed to contain one of the statically
        // defined presence primitives ("required", "optional", or "repeated"). Since their
        // byte lengths are small constants, casting to u32 can never cause a runtime truncation.
        #[allow(clippy::cast_possible_truncation)]
        let keyword_len = keyword.len() as u32;

        let range = Range {
            end: Position {
                character: block_range.start.character + keyword_len,
                ..block_range.end
            },
            ..block_range
        };

        Some(Self { kind, range })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use async_lsp::lsp_types::{Position, Range};

    #[test]
    fn test_field_cardinality_extraction_success() {
        let block_range = Range {
            start: Position {
                line: 5,
                character: 2,
            },
            end: Position {
                line: 5,
                character: 27,
            },
        };

        let req =
            FieldCardinality::from_block_text("required string name = 1;", block_range).unwrap();
        assert!(matches!(req.kind, CardinalityKind::Required));
        assert_eq!(req.range.start.character, 2);
        assert_eq!(req.range.end.character, 10);

        let opt =
            FieldCardinality::from_block_text("  optional int32 id = 2;", block_range).unwrap();
        assert!(matches!(opt.kind, CardinalityKind::Optional));
        assert_eq!(opt.range.start.character, 2);
        assert_eq!(opt.range.end.character, 10);

        let rep =
            FieldCardinality::from_block_text("repeated bytes data = 3;", block_range).unwrap();
        assert!(matches!(rep.kind, CardinalityKind::Repeated));
        assert_eq!(rep.range.end.character, 10);
    }

    #[test]
    fn test_field_cardinality_implicit_or_invalid() {
        let block_range = Range {
            start: Position {
                line: 10,
                character: 4,
            },
            end: Position {
                line: 10,
                character: 25,
            },
        };

        let implicit = FieldCardinality::from_block_text("string global_id = 1;", block_range);
        assert!(implicit.is_none());

        let invalid =
            FieldCardinality::from_block_text("unknown_label string identity = 1;", block_range);
        assert!(invalid.is_none());

        assert!(FieldCardinality::from_block_text("", block_range).is_none());
    }
}
