//! Cross-file name resolution engine.
//!
//! This module implements the name resolution layer that links type references
//! to their definitions across files using fully-qualified names (FQNs), fully
//! decoupling features like go-to-definition, hover, and rename from raw
//! syntax-document node types.

use std::collections::BTreeMap;

use async_lsp::lsp_types::{Location, Position, TextEdit, Url};

use crate::model::{ModelElement, SpatialEntry, TypeReference};
use crate::state::ProtoLanguageState;
use crate::utils::{is_position_inside_range, trailing_segment};

/// A resolved definition target located anywhere in the workspace.
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    /// The document containing the definition.
    pub uri: Url,
    /// The metamodel element defining the symbol.
    pub element: ModelElement,
}

impl ProtoLanguageState {
    /// Performs tiered protobuf name resolution for a reference identifier.
    ///
    /// Resolution order:
    /// 1. **Fully-qualified** names (prefixed with `.`) — matched exactly.
    /// 2. **Lexical scope chain** — walk the enclosing scope prefixes from
    ///    innermost to outermost, trying `{scope}.{reference}` for each.
    /// 3. **Loose suffix fallback** — for short names defined inside a nested
    ///    container that aren't reachable through the direct scope chain.
    pub fn resolve_reference(&self, scope: &str, reference: &str) -> Vec<ResolvedTarget> {
        if let Some(fqn) = reference.strip_prefix('.') {
            return self.lookup_fqn(fqn);
        }

        for prefix in scope_prefixes(scope) {
            let fqn = if prefix.is_empty() {
                reference.to_string()
            } else {
                format!("{prefix}.{reference}")
            };
            let matches = self.lookup_fqn(&fqn);
            if !matches.is_empty() {
                return matches;
            }
        }

        self.lookup_fqn_suffix(reference)
    }

    /// Finds every element whose Fully Qualified Name equals `fqn`.
    fn lookup_fqn(&self, fqn: &str) -> Vec<ResolvedTarget> {
        let mut out = Vec::new();
        for document in self.get_documents() {
            for element in &document.elements {
                if element.kind.fqn() == Some(fqn) {
                    out.push(ResolvedTarget {
                        uri: document.uri.clone(),
                        element: element.clone(),
                    });
                }
            }
        }
        out
    }

    /// Finds every element whose FQN is exactly `name` or ends with a `.name`
    /// component boundary (avoids partial identifier matches).
    fn lookup_fqn_suffix(&self, name: &str) -> Vec<ResolvedTarget> {
        let boundary = format!(".{name}");
        let mut out = Vec::new();
        for document in self.get_documents() {
            for element in &document.elements {
                if let Some(fqn) = element.kind.fqn()
                    && (fqn == name || fqn.ends_with(&boundary))
                {
                    out.push(ResolvedTarget {
                        uri: document.uri.clone(),
                        element: element.clone(),
                    });
                }
            }
        }
        out
    }

    /// Resolves the fully-qualified name of the symbol under `position`.
    ///
    /// If the cursor rests on a declaration name, the symbol's own FQN is
    /// returned. If it rests on a type reference, the segment under the cursor
    /// is resolved to its referenced definition's FQN (so renaming the outer
    /// segment of `Book.Author` targets `Book`, not the nested `Author`).
    pub fn resolve_target_fqn(&self, uri: &Url, position: Position) -> Option<String> {
        let document = self.get_document(uri)?;
        let SpatialEntry { element_id, .. } = document.find_entry_at_position(position)?;
        let element = document.elements.get(*element_id)?;

        if is_position_inside_range(position, element.meta.selection_range) {
            return element.kind.fqn().map(ToOwned::to_owned);
        }

        let type_ref = element.type_reference_at(position)?;
        let scope = element.kind.fqn().unwrap_or(&document.package);
        let ref_path = type_ref_segment_prefix(type_ref, position)?;
        self.resolve_reference(scope, &ref_path)
            .into_iter()
            .next()
            .and_then(|target| target.element.kind.fqn().map(ToOwned::to_owned))
    }

    /// Returns the declaration location (URI + name position) of the first
    /// element matching `target_fqn` in the indexed workspace.
    pub fn declaration_for_fqn(&self, target_fqn: &str) -> Option<(Url, Position)> {
        for document in self.get_documents() {
            for element in &document.elements {
                if element.kind.fqn() == Some(target_fqn) {
                    return Some((document.uri.clone(), element.meta.selection_range.start));
                }
            }
        }
        None
    }

    /// Collects every reference site for a symbol identified by its FQN across
    /// the indexed workspace: all matching declarations plus every type
    /// reference that resolves back to the same FQN.
    pub fn references_for_fqn(&self, target_fqn: &str) -> Vec<Location> {
        let mut refs = Vec::new();
        for document in self.get_documents() {
            for element in &document.elements {
                if element.kind.fqn() == Some(target_fqn) {
                    refs.push(Location {
                        uri: document.uri.clone(),
                        range: element.meta.selection_range,
                    });
                }
                let scope = element.kind.fqn().unwrap_or(&document.package);
                for type_ref in element.kind.type_references() {
                    if self
                        .resolve_reference(scope, &type_ref.name)
                        .iter()
                        .any(|r| r.element.kind.fqn() == Some(target_fqn))
                    {
                        refs.push(Location {
                            uri: document.uri.clone(),
                            range: type_ref.range,
                        });
                    }
                }
            }
        }
        // Deterministic output ordering for stable snapshots / tests.
        refs.sort_by_key(|l| {
            (
                l.uri.as_str().to_string(),
                l.range.start.line,
                l.range.start.character,
            )
        });
        refs
    }

    /// Produces rename edits that update the declaration(s) and every reference
    /// site for a symbol identified by its FQN to `new_name`.
    ///
    /// Beyond sites that resolve *exactly* to `target_fqn`, this also rewrites
    /// references to types nested underneath it (e.g. renaming `Author` to
    /// `Writer` also updates `Author.Address` → `Writer.Address`), since the
    /// nested qualification shifts with the enclosing message.
    pub fn rename_for_fqn(&self, target_fqn: &str, new_name: &str) -> BTreeMap<Url, Vec<TextEdit>> {
        let old_simple = trailing_segment(target_fqn);
        let nested_prefix = format!("{target_fqn}.");
        let mut edits: BTreeMap<Url, Vec<TextEdit>> = BTreeMap::new();

        for document in self.get_documents() {
            for element in &document.elements {
                if element.kind.fqn() == Some(target_fqn) {
                    edits
                        .entry(document.uri.clone())
                        .or_default()
                        .push(TextEdit {
                            range: element.meta.selection_range,
                            new_text: new_name.to_owned(),
                        });
                }

                let scope = element.kind.fqn().unwrap_or(&document.package);
                for type_ref in element.kind.type_references() {
                    let resolves = self.resolve_reference(scope, &type_ref.name);
                    let is_target = resolves.iter().any(|r| {
                        r.element.kind.fqn() == Some(target_fqn)
                    });
                    let is_nested = resolves.iter().any(|r| {
                        r.element
                            .kind
                            .fqn()
                            .is_some_and(|fqn| fqn.starts_with(&nested_prefix))
                    });
                    if is_target || is_nested {
                        edits.entry(document.uri.clone()).or_default().push(TextEdit {
                            range: type_ref.range,
                            new_text: rename_reference_text(
                                &type_ref.name,
                                old_simple,
                                new_name,
                            ),
                        });
                    }
                }
            }
        }

        // Deterministic output ordering for stable snapshots / tests.
        for edits_in_file in edits.values_mut() {
            edits_in_file.sort_by_key(|e| {
                (
                    e.range.start.line,
                    e.range.start.character,
                    e.range.end.line,
                    e.range.end.character,
                )
            });
        }
        edits
    }
}

/// Produces the chain of enclosing scope prefixes for a scope FQN, from the
/// innermost scope down to the empty (root) scope.
///
/// `com.example.Book` yields `["com.example.Book", "com.example", "com", ""]`.
fn scope_prefixes(scope: &str) -> Vec<&str> {
    let mut prefixes = Vec::new();
    let mut current = scope;
    loop {
        prefixes.push(current);
        match current.rfind('.') {
            Some(idx) => current = &current[..idx],
            None => break,
        }
    }
    prefixes
}

/// Rewrites the segment of a type reference that names the renamed type to
/// `new_name`, preserving any surrounding qualification prefix and nested
/// suffix.
///
/// `rename_reference_text("Author.Address", "Author", "Writer")` yields
/// `"Writer.Address"`; `"com.workspace.Author"` becomes
/// `"com.workspace.Writer"`; an unqualified `"Author"` becomes `"Writer"`.
fn rename_reference_text(name: &str, old_simple: &str, new_name: &str) -> String {
    let mut parts: Vec<&str> = name.split('.').collect();
    if let Some(idx) = parts.iter().position(|p| *p == old_simple) {
        parts[idx] = new_name;
    }
    parts.join(".")
}

/// Returns the dot-joined path of a type reference up to and including the
/// segment the cursor rests on.
///
/// For `Outer.Inner` with the cursor on `Inner`, returns `"Outer.Inner"`.
fn type_ref_segment_prefix(type_ref: &TypeReference, position: Position) -> Option<String> {
    if position.line != type_ref.range.start.line {
        return None;
    }
    let mut cursor = type_ref.range.start.character as usize;
    let position_char = position.character as usize;
    let mut prefix: Vec<&str> = Vec::new();
    for segment in type_ref.name.split('.') {
        prefix.push(segment);
        let seg_end = cursor + segment.len();
        if position_char < seg_end {
            return Some(prefix.join("."));
        }
        cursor = seg_end + 1; // skip the '.' separator
    }
    None
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use std::path::PathBuf;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;

    fn setup() -> ProtoLanguageState {
        let ipath = vec![PathBuf::from("src/state/input")];
        let a_uri: Url = "file://input/a.proto".parse().unwrap();
        let b_uri: Url = "file://input/b.proto".parse().unwrap();
        let c_uri: Url = "file://input/c.proto".parse().unwrap();

        let mut state = ProtoLanguageState::new();
        state.upsert_file(&a_uri, include_str!("input/a.proto"), &ipath, 2, &Config::default(), false);
        state.upsert_file(&b_uri, include_str!("input/b.proto"), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, include_str!("input/c.proto"), &ipath, 2, &Config::default(), false);
        state
    }

    fn fqns(state: &ProtoLanguageState, scope: &str, reference: &str) -> Vec<String> {
        state
            .resolve_reference(scope, reference)
            .into_iter()
            .map(|t| t.element.kind.fqn().unwrap_or_default().to_string())
            .collect()
    }

    #[test]
    fn test_resolve_scope_chain() {
        let state = setup();
        assert_eq!(
            fqns(&state, "com.workspace", "Author"),
            vec!["com.workspace.Author"]
        );
        assert_eq!(
            fqns(&state, "com.workspace", "Author.Address"),
            vec!["com.workspace.Author.Address"]
        );
        assert_eq!(
            fqns(&state, "com.workspace", "com.utility.Foobar.Baz"),
            vec!["com.utility.Foobar.Baz"]
        );
    }

    #[test]
    fn test_resolve_suffix_fallback() {
        let state = setup();
        // Baz is nested inside Foobar and not on the direct scope chain.
        assert_eq!(
            fqns(&state, "com.utility", "Baz"),
            vec!["com.utility.Foobar.Baz"]
        );
    }

    #[test]
    fn test_resolve_fully_qualified() {
        let state = setup();
        assert_eq!(
            fqns(&state, "com.workspace", ".com.utility.Foobar.Baz"),
            vec!["com.utility.Foobar.Baz"]
        );
    }

    #[test]
    fn test_resolve_boundary_avoids_partial_identifier() {
        let mut state = ProtoLanguageState::new();
        let ipath: &[PathBuf] = &[];
        let uri: Url = "file:///t.proto".parse().unwrap();
        let content = "syntax = \"proto3\";\npackage com.test;\nmessage FooBaz { int32 x = 1; }\n";
        state.upsert_file(&uri, content, ipath, 1, &Config::default(), false);

        // "Baz" is only a partial suffix of FooBaz -> must not resolve.
        assert!(state.resolve_reference("com.test", "Baz").is_empty());
        assert_eq!(fqns(&state, "com.test", "FooBaz"), vec!["com.test.FooBaz"]);
    }

    #[test]
    fn test_scope_prefixes() {
        assert_eq!(
            super::scope_prefixes("com.example.Book"),
            vec!["com.example.Book", "com.example", "com"]
        );
        assert_eq!(super::scope_prefixes("com.workspace"), vec!["com.workspace", "com"]);
        assert_eq!(super::scope_prefixes(""), vec![""]);
    }

    #[test]
    fn test_rename_reference_text() {
        // Unqualified.
        assert_eq!(
            super::rename_reference_text("Author", "Author", "Writer"),
            "Writer"
        );
        // Nested-qualified (leading segment renamed).
        assert_eq!(
            super::rename_reference_text("Author.Address", "Author", "Writer"),
            "Writer.Address"
        );
        // Fully package-qualified (middle segment renamed).
        assert_eq!(
            super::rename_reference_text(
                "com.utility.Foobar.Baz",
                "Baz",
                "Baaz"
            ),
            "com.utility.Foobar.Baaz"
        );
    }

    #[test]
    fn test_resolve_relative_and_leading_dot() {
        let state = setup();
        // Relative reference resolved against the current package.
        assert_eq!(
            fqns(&state, "com.workspace.Book", "Author"),
            vec!["com.workspace.Author"]
        );
        // Explicit leading-dot fully-qualified name.
        assert_eq!(
            fqns(&state, "com.workspace", ".com.utility.Foobar.Baz"),
            vec!["com.utility.Foobar.Baz"]
        );
    }
}
