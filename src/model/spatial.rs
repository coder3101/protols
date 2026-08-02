//! Spatial navigation and coordinate lookup layer for the protobuf metamodel.
//!
//! This module implements the compilation mechanics for the coordinate index.
//!
//! It maps raw physical text ranges on disk to internal numerical metamodel
//! element IDs.

use async_lsp::lsp_types::Position;

use crate::{model::TypeReference, utils::is_position_inside_range};

use super::types::{ElementKind, ModelElement, SpatialEntry};

impl ElementKind {
    /// Returns every type reference embedded in this element kind (field
    /// types, map key/value types, and RPC request/response types).
    pub fn type_references(&self) -> Vec<&TypeReference> {
        match self {
            ElementKind::Field { type_ref, .. } | ElementKind::OneofField { type_ref, .. } => {
                vec![type_ref]
            }
            ElementKind::MapField {
                key_type_ref,
                value_type_ref,
                ..
            } => vec![key_type_ref, value_type_ref],
            ElementKind::Rpc {
                request_type_ref,
                response_type_ref,
                ..
            } => vec![request_type_ref, response_type_ref],
            _ => Vec::new(),
        }
    }
}

impl ModelElement {
    /// Returns the type reference whose geometric bounds contain `position`,
    /// if any.
    pub fn type_reference_at(&self, position: Position) -> Option<&TypeReference> {
        self.kind
            .type_references()
            .into_iter()
            .find(|r| is_position_inside_range(position, r.range))
    }
}

impl ModelElement {
    /// Flattens the element's internal name boundaries and type reference
    /// bounds into autonomous geometric intersection entries.
    ///
    /// This method populates the shared lookup index by pushing a primary entry
    /// for the element's own `selection_range`, followed by targeted entries
    /// for any embedded type reference ranges (such as fields, maps, or RPC
    /// endpoints) discovered via `TypeReference`.
    ///
    /// # Arguments
    ///
    /// * `index` - A mutable reference to the flat vector housing all compiled
    ///   spatial index blocks.
    #[inline]
    pub fn collect_spatial_entries(&self, index: &mut Vec<SpatialEntry>) {
        let element_id = self.id;

        index.push(SpatialEntry {
            range: self.meta.selection_range,
            element_id,
        });

        let references: &[&TypeReference] = match &self.kind {
            ElementKind::Field { type_ref, .. } | ElementKind::OneofField { type_ref, .. } => {
                &[type_ref]
            }
            ElementKind::MapField {
                key_type_ref,
                value_type_ref,
                ..
            } => &[key_type_ref, value_type_ref],
            ElementKind::Rpc {
                request_type_ref,
                response_type_ref,
                ..
            } => &[request_type_ref, response_type_ref],
            _ => &[],
        };

        for TypeReference { range, .. } in references {
            index.push(SpatialEntry {
                range: *range,
                element_id,
            });
        }
    }
}

impl SpatialEntry {
    /// Evaluates whether the requested LSP position is situated inclusively
    /// within the physical boundaries of this specific spatial intersection
    /// block.
    #[inline]
    pub fn contains_position(&self, position: Position) -> bool {
        is_position_inside_range(position, self.range)
    }
}

impl ModelElement {
    /// Checks whether the specified LSP position lies strictly within the
    /// full anatomical text boundaries of this element.
    ///
    /// This is a two-sided inclusive boundary check that guarantees the point is
    /// after the start and before the end of the element's entire range.
    #[inline]
    pub fn contains_position(&self, position: Position) -> bool {
        is_position_inside_range(position, self.meta.range)
    }

    /// Checks whether the element encompasses the given position by ensuring
    /// that the parser has not yet moved past the element's lower boundary.
    ///
    /// This is a fast, one-sided geometric boundary check (`range.end >= point`).
    /// It assumes the parser moves strictly downwards from the top of the file,
    /// making it ideal for managing the context hierarchy stack to determine
    /// if the current active container has closed.
    #[inline]
    pub fn encloses_point(&self, point: Position) -> bool {
        self.meta.range.end >= point
    }
}
