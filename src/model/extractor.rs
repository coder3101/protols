//! High-performance metadata extractor pipeline for protobuf schemas.
//!
//! This module coordinates the execution of compiled Tree-sitter queries
//! against the source abstract syntax document (AST). It handles sequential stream
//! matching, manages the container hierarchy stack, accumulates floating
//! docstrings, and compiles the final elements registry.

pub use query::generate_metamodel_query;

use async_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use super::captures;
use super::captures::definitions;
use super::types::{CommentBlock, ElementKind, ElementMeta, ModelElement};

mod handlers;
mod query;

/// Executes the compiled Tree-sitter query against the syntax document root to
/// build the pure-memory metamodel registry.
///
/// This function acts as the orchestrator of the extraction pipeline. It runs a
/// `QueryCursor` top-down through the syntax document nodes, captures matched
/// patterns, dispatches them to specialized structural handlers, and
/// incrementally feeds the results into the chronological context building
/// loop.
///
/// # Arguments
///
/// * `root_node` - The top-level abstract syntax document [`Node`] representing the
///   fully parsed document.
/// * `source` - The raw UTF-8 byte array sequence containing the complete
///   original source file content on disk.
/// * `query` - The pre-compiled [`Query`] instance.
///
/// # Returns
///
/// Returns a tuple containing:
/// 1. A [`String`] representing the extracted package namespace of the file
///    (defaults to empty if missing).
/// 2. A [`Vec<ModelElement>`] containing the completely assembled, flat
///    hierarchical graph registry of all elements.
pub fn build_meta_model(
    root_node: Node,
    source: &[u8],
    query: &Query,
) -> (String, Vec<ModelElement>) {
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, root_node, source);
    let capture_names = query.capture_names();
    let mut package_name = None;
    let mut parsed_matches: Vec<ParsedMatch> = Vec::with_capacity(128);

    while let Some(query_match) = matches.next() {
        let kind_str = query_match.captures.iter().copied().find_map(|c| {
            let name = capture_names[c.index as usize];
            definitions::is_match(name).then_some(name)
        });

        let mut element = handlers::dispatch_element(kind_str, query_match, capture_names, source);

        if let Some(ParsedMatch::Package { name }) =
            element.take_if(|e| matches!(e, ParsedMatch::Package { .. }))
        {
            package_name.get_or_insert(name);
        }

        parsed_matches.extend(element);
    }

    let mut elements: Vec<ModelElement> = Vec::with_capacity(parsed_matches.len());
    let mut context_stack: Vec<usize> = Vec::new();
    let mut documentation_buffer: Vec<CommentBlock> = Vec::new();
    let package_name = package_name.unwrap_or_default();

    parsed_matches.sort_by_key(|m| m.range().start);

    for parsed_match in parsed_matches {
        parsed_match.process(
            &mut elements,
            &mut context_stack,
            &mut documentation_buffer,
            &package_name,
        );
    }

    (package_name, elements)
}

#[inline]
fn is_adjacent_documentation(
    documentation_buffer: &[CommentBlock],
    entity_start_line: u32,
) -> bool {
    documentation_buffer
        .last()
        .is_some_and(|c| entity_start_line.saturating_sub(c.range.end.line) == 1)
}

#[inline]
fn build_fqn(
    context_stack: &[usize],
    elements: &[ModelElement],
    entity_name: &str,
    package_name: &str,
) -> String {
    let parent_fqn = context_stack
        .iter()
        .copied()
        .rev()
        .filter_map(|id| elements.get(id))
        .find_map(|parent| parent.kind.namespace_fqn());

    match parent_fqn {
        Some(fqn) => format!("{fqn}.{entity_name}"),
        None if package_name.is_empty() => entity_name.to_string(),
        None => format!("{package_name}.{entity_name}"),
    }
}

/// Represents an intermediate semantic unit yielded by specialized syntax
/// extraction handlers.
///
/// These variants act as asynchronous atomic updates that are processed
/// sequentially by the building loop to gradually assemble the final state of
/// the metamodel.
#[derive(Debug)]
pub(super) enum ParsedMatch {
    /// A standalone block comment or leading documentation string.
    Comment(CommentBlock),

    /// A package namespace declaration defining the default scope prefix of the
    /// file.
    Package { name: String },

    /// An option marker signaling that the target container or terminal field
    /// is deprecated.
    DeprecationMarker { range: Range },

    /// A concrete structural entity declaration (e.g., messages, fields,
    /// RPC endpoints).
    Entity {
        kind: ElementKind,
        range: Range,
        selection_range: Range,
    },
}

impl ParsedMatch {
    /// Extracts the underlying physical source boundaries of the parsed match
    /// sequence.
    ///
    /// This geometric layout reference isolates the coordinate ranges across
    /// distinct variants, providing a uniform anchor point used primarily to
    /// sort non-linear document-sitter streaming buffers back into topological
    /// top-down code order.
    ///
    /// # Returns
    ///
    /// Returns the exact [`Range`] spanning the token's presence in the file.
    /// [`ParsedMatch::Package`] defaults to an empty zero-range positioned at
    /// the absolute start of the document.
    fn range(&self) -> Range {
        match self {
            Self::Comment(c) => c.range,
            Self::DeprecationMarker { range } | Self::Entity { range, .. } => *range,
            Self::Package { .. } => Range::default(),
        }
    }

    /// Sequentially processes the single parsed token update.
    ///
    /// This method manages adjacent docstring buffers via exclusive geometric
    /// line-affinity rules, dynamically injects fully qualified names (FQN),
    /// and links children nodes to their current active parent containers.
    #[inline]
    fn process(
        self,
        elements: &mut Vec<ModelElement>,
        context_stack: &mut Vec<usize>,
        documentation_buffer: &mut Vec<CommentBlock>,
        package_name: &str,
    ) {
        match self {
            Self::Package { .. } => {}
            Self::Comment(comment) => {
                if is_inline_trailing(elements, comment.range.start.line) {
                    return;
                }

                if !is_adjacent_documentation(documentation_buffer, comment.range.start.line) {
                    documentation_buffer.clear();
                }

                documentation_buffer.push(comment);
            }
            Self::DeprecationMarker { range } => {
                let target_id = elements
                    .last()
                    .filter(|e| e.contains_position(range.start) && e.kind.is_terminal())
                    .map(|e| e.id)
                    .or_else(|| context_stack.last().copied());

                if let Some(target) = target_id.and_then(|idx| elements.get_mut(idx)) {
                    target.kind.set_deprecated(true);
                }
            }
            Self::Entity {
                mut kind,
                range,
                selection_range,
            } => {
                let id = elements.len();

                prune_context_stack(context_stack, elements, range.start);

                let documentation =
                    if is_adjacent_documentation(documentation_buffer, range.start.line) {
                        // https://rust-lang.github.io/rust-clippy/rust-1.96.0/index.html#drain_collect
                        // Allow: Clippy suggests std::mem::take(), but that resets capacity to 0.
                        // We use drain(..).collect() here to keep the underlying allocation
                        // of `documentation_buffer` intact for reuse in the parsing loop.
                        #[allow(clippy::drain_collect)]
                        documentation_buffer.drain(..).collect()
                    } else {
                        documentation_buffer.clear();
                        Vec::new()
                    };

                let name = kind.fqn().unwrap_or_default().to_string();

                let full_fqn = build_fqn(context_stack, elements, &name, package_name);
                kind.set_fqn(full_fqn);

                let is_container = !kind.is_terminal();
                let parent_id = context_stack.last().copied();

                elements.push(ModelElement {
                    id,
                    parent_id,
                    meta: ElementMeta {
                        name,
                        range,
                        selection_range,
                        documentation,
                    },
                    kind,
                    children: Vec::new(),
                });

                if let Some(parent) = parent_id.and_then(|p_idx| elements.get_mut(p_idx)) {
                    parent.children.push(id);
                }

                if is_container {
                    context_stack.push(id);
                }
            }
        }
    }
}

#[inline]
fn prune_context_stack(
    context_stack: &mut Vec<usize>,
    elements: &[ModelElement],
    current_position: Position,
) {
    while let Some(parent_idx) = context_stack.last().copied() {
        let is_active = elements
            .get(parent_idx)
            .is_some_and(|parent| parent.encloses_point(current_position));

        if is_active {
            return;
        }
        context_stack.pop();
    }
}

/// Checks if the given comment line rests on the exact same physical line where
/// any active element or its parent hierarchical container closes.
#[inline]
fn is_inline_trailing(elements: &[ModelElement], comment_line: u32) -> bool {
    std::iter::successors(elements.last(), |current| {
        current
            .parent_id
            .and_then(|parent_idx| elements.get(parent_idx))
    })
    .any(|e| e.meta.range.end.line == comment_line)
}
