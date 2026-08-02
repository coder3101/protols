//! Unified internal metamodel and spatial coordination domain for protobuf
//! schemas.
//!
//! This module serves as the root and entry point for all pure-memory data
//! caching layers within the language server. It aggregates the semantic schema
//! entities, geometric coordinate indices, textual extraction pipelines, and
//! presentations tools.
//!
//! # Module Architecture
//!
//! * [`types`] - Pure domain entities, structural nodes (`ModelElement`), and
//!   schema flavors (`ElementKind`).
//! * [`spatial`] - Coordinate navigation layers, indexing logic, and flat
//!   bidirectional geometry checks.
//! * [`extractor`] - Sequential Tree-sitter query runners, handler routing, and
//!   context stack.
//! * [`captures`] - Global string constant mappings binding the SCM query
//!   handles to the Rust runtime.
//! * [`mutation`] - Safe in-place graph modifiers, zero-copy buffers, and
//!   string parsing implementations.
//! * [`presentation`] - Formatting engines compiling markdown tooltips and rich
//!   LSP hover signatures.

pub use extractor::{build_meta_model, generate_metamodel_query};
pub use types::*;

mod captures;
mod extractor;
mod mutation;
mod presentation;
mod spatial;
mod types;
