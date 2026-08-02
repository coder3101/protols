//! Presentation and formatting layer for protobuf metamodel elements.
//!
//! This module coordinates the translation of memory-backed semantic entities
//! into formatted text blocks, markdown structures, and human-readable string
//! representations.

use std::fmt;
use std::fmt::Write;

use async_lsp::lsp_types::{Position, SymbolKind};

use crate::docs;

use super::types::{CardinalityKind, ElementKind, ModelElement, TypeReference};

impl From<&ElementKind> for SymbolKind {
    /// Maps an internal [`ElementKind`] variant directly to its closest
    /// semantic LSP [`SymbolKind`].
    fn from(kind: &ElementKind) -> Self {
        match kind {
            ElementKind::Import { .. } => Self::MODULE,
            ElementKind::Message { .. } => Self::STRUCT,
            ElementKind::Oneof { .. } => Self::OBJECT,
            ElementKind::Field { .. }
            | ElementKind::MapField { .. }
            | ElementKind::OneofField { .. } => Self::FIELD,
            ElementKind::Enum { .. } => Self::ENUM,
            ElementKind::EnumValue { .. } => Self::ENUM_MEMBER,
            ElementKind::Service { .. } => Self::INTERFACE,
            ElementKind::Rpc { .. } => Self::METHOD,
        }
    }
}

impl ModelElement {
    const DEPRECATED_BANNER: &'static str = "**`Deprecated`**\n";
    const CODE_BLOCK_START: &'static str = "```protobuf\n";
    const CODE_BLOCK_END: &'static str = "\n```";
    const SEPARATOR: &'static str = "\n\n---";
    const BASE_PROTOBUF_KEYWORDS_LEN: usize = 48;

    /// Compiles the complete user-facing markdown text representation for this
    /// element.
    ///
    /// If the cursor rests on a builtin datatype, it short-circuits to render
    /// the raw documentation string. Otherwise, it extracts the clean
    /// fenced-code syntax signature from `ElementKind` and appends all adjacent
    /// accumulated leading comments.
    pub fn to_hover_markdown(&self, position: Position) -> Option<String> {
        if let Some(referenced_type) = self.inspect_nested_type_reference(position) {
            // Returns `None` for user-defined types.
            return docs::BUILTIN
                .get(referenced_type)
                .or_else(|| crate::docs::WELLKNOWN.get(referenced_type))
                .map(ToString::to_string);
        }

        let mut sig_markdown_overhead = Self::CODE_BLOCK_START.len() + Self::CODE_BLOCK_END.len();

        if self.kind.is_deprecated() {
            sig_markdown_overhead += Self::DEPRECATED_BANNER.len();
        }

        let fqn_len = self.kind.fqn().map(|fqn| fqn.len() + 1).unwrap_or_default();

        let expected_sig_len = sig_markdown_overhead
            + fqn_len
            + self.meta.name.len()
            + Self::BASE_PROTOBUF_KEYWORDS_LEN;

        let expected_docs_len = if self.meta.documentation.is_empty() {
            0
        } else {
            Self::SEPARATOR.len()
                + self
                    .meta
                    .documentation
                    .iter()
                    .map(|comment| 1 + comment.text.len())
                    .sum::<usize>()
        };

        let mut hover_text = String::with_capacity(expected_sig_len + expected_docs_len);

        self.fill_protobuf_signature(&mut hover_text)?;

        if self.meta.documentation.is_empty() {
            return Some(hover_text);
        }

        hover_text.push_str(Self::SEPARATOR);

        for comment in &self.meta.documentation {
            hover_text.push('\n');
            hover_text.push_str(comment.text.as_str());
        }

        Some(hover_text)
    }

    /// Renders a dense, markdown-fenced protobuf syntax block capturing the
    /// comprehensive signature definition of this specific schema element
    /// into the provided pre-allocated string buffer.
    ///
    /// It appends a **`Deprecated`** warning banner if the element's metadata
    /// configuration requires it, and falls back to a descriptive format for
    /// imports.
    ///
    /// # Arguments
    ///
    /// * `buffer` - A mutable reference to an existing `String` buffer where
    ///   the compiled markdown syntax block will be appended in-place without
    ///   triggering reallocations.
    ///
    /// # Returns
    ///
    /// Returns `Some(())` if the signature body was successfully evaluated and
    /// fully appended to the buffer, or `None` if the element represents an
    /// internal, non-renderable state (such as a generic fallback import type).
    pub fn fill_protobuf_signature(&self, buffer: &mut String) -> Option<()> {
        if let ElementKind::Import { path } = &self.kind {
            const PREFIX: &str = "Import: `";
            const SUFFIX: &str = "` protobuf file";

            buffer.push_str(PREFIX);
            buffer.push_str(path);
            buffer.push_str(SUFFIX);

            return Some(());
        }

        if self.kind.is_deprecated() {
            buffer.push_str(Self::DEPRECATED_BANNER);
        }

        buffer.push_str(Self::CODE_BLOCK_START);

        if let Some(fqn) = self.kind.fqn() {
            let _ = writeln!(buffer, "{fqn}");
        }

        let element_name = &self.meta.name;

        match &self.kind {
            ElementKind::Message { .. } => {
                let _ = write!(buffer, "message {element_name}");
            }
            ElementKind::Field {
                type_ref,
                cardinality,
                tag,
                ..
            } => {
                if let Some(c) = cardinality {
                    let _ = write!(buffer, "{} ", c.kind);
                }
                let _ = write!(buffer, "{} {element_name} = {tag};", type_ref.name);
            }
            ElementKind::OneofField {
                type_ref: TypeReference { name, .. },
                tag,
                ..
            } => {
                let _ = write!(buffer, "{name} {element_name} = {tag};");
            }
            ElementKind::MapField {
                key_type_ref,
                value_type_ref,
                tag,
                ..
            } => {
                let _ = write!(
                    buffer,
                    "map<{}, {}> {element_name} = {tag};",
                    key_type_ref.name, value_type_ref.name
                );
            }
            ElementKind::Oneof { .. } => {
                let _ = write!(buffer, "oneof {element_name}");
            }
            ElementKind::Enum { .. } => {
                let _ = write!(buffer, "enum {element_name}");
            }
            ElementKind::EnumValue { number, .. } => {
                let _ = write!(buffer, "{element_name} = {number};");
            }
            ElementKind::Service { .. } => {
                let _ = write!(buffer, "service {element_name}");
            }
            ElementKind::Rpc {
                request_type_ref,
                request_stream,
                response_type_ref,
                response_stream,
                ..
            } => {
                let request_prefix = request_stream
                    .as_ref()
                    .map(|_| "stream ")
                    .unwrap_or_default();
                let response_prefix = response_stream
                    .as_ref()
                    .map(|_| "stream ")
                    .unwrap_or_default();

                let _ = write!(
                    buffer,
                    "rpc {element_name}({request_prefix}{}) returns ({response_prefix}{});",
                    request_type_ref.name, response_type_ref.name
                );
            }
            ElementKind::Import { .. } => return None,
        }

        buffer.push_str(Self::CODE_BLOCK_END);

        Some(())
    }
}

impl ElementKind {
    /// Extracts the Fully Qualified Name (FQN) from container or terminal
    /// elements that possess a valid hierarchical namespace prefix.
    ///
    /// Returns `Some(&str)` representing the structural absolute or relative
    /// FQN path, or `None` if the entity operates outside the named scope (like
    /// `Package` or `Import`).
    #[inline]
    pub fn fqn(&self) -> Option<&str> {
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
            return Some(fqn);
        }

        None
    }

    /// Returns the FQN of the element only if it acts as a structural
    /// namespace container for child elements according to the Protobuf spec
    /// (e.g., Messages, Enums, Services).
    ///
    /// Elements like `Oneof` are excluded because they do not participate
    /// in the FQN resolution path.
    #[inline]
    pub fn namespace_fqn(&self) -> Option<&str> {
        match self {
            Self::Message { fqn, .. } | Self::Enum { fqn, .. } | Self::Service { fqn, .. } => {
                Some(fqn)
            }
            _ => None,
        }
    }

    /// Evaluates whether the element is flagged as deprecated.
    #[inline]
    pub const fn is_deprecated(&self) -> bool {
        match self {
            Self::Field { is_deprecated, .. }
            | Self::OneofField { is_deprecated, .. }
            | Self::MapField { is_deprecated, .. }
            | Self::Rpc { is_deprecated, .. }
            | Self::EnumValue { is_deprecated, .. }
            | Self::Message { is_deprecated, .. }
            | Self::Enum { is_deprecated, .. }
            | Self::Service { is_deprecated, .. } => *is_deprecated,

            Self::Oneof { .. } | Self::Import { .. } => false,
        }
    }

    /// Determines whether the element represents a leaf node (terminal) in the
    /// hierarchy, meaning it cannot syntactically contain any nested
    /// sub-elements.
    #[inline]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Import { .. }
                | Self::Field { .. }
                | Self::MapField { .. }
                | Self::OneofField { .. }
                | Self::EnumValue { .. }
                | Self::Rpc { .. }
        )
    }
}

impl fmt::Display for CardinalityKind {
    /// Formats the enum variant into its canonical, lowercase protobuf keyword
    /// token string sequence.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::Repeated => "repeated",
            Self::Optional => "optional",
            Self::Required => "required",
        };
        f.write_str(s)
    }
}
