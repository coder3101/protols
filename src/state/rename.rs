use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use async_lsp::lsp_types::{Location, Position, TextEdit, Url};

use crate::model::ElementKind;
use crate::state::ProtoLanguageState;
use crate::utils::{is_position_inside_range, trailing_segment};

/// A single rename operation to apply against the workspace: rename whatever
/// symbol is declared at `(uri, pos)` to `new_name`. Multiple ops are merged
/// into one `WorkspaceEdit` when a single user invocation triggers chained
/// renames (e.g. rpc + request + response).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameOp {
    pub uri: Url,
    pub pos: Position,
    pub new_name: String,
}

impl ProtoLanguageState {
    /// Find every rpc declaration in the workspace whose simple name matches
    /// `rpc_name`. Used by the rpc/request/response chain rename to enumerate
    /// candidate rpcs when the user invokes rename on a convention-named
    /// message; the caller then narrows the list by checking which candidate
    /// actually references the user's primary message.
    pub fn find_rpc_decls(&self, rpc_name: &str) -> Vec<Location> {
        let mut out = vec![];
        for document in self.get_documents() {
            for element in &document.elements {
                if matches!(element.kind, ElementKind::Rpc { .. }) && element.meta.name == rpc_name
                {
                    out.push(Location {
                        uri: document.uri.clone(),
                        range: element.meta.selection_range,
                    });
                }
            }
        }
        out
    }

    /// Count the number of rpcs in the workspace whose request or response
    /// type's trailing identifier segment matches `type_simple_name`. Used for
    /// the uniqueness check before chain-renaming a request/response message.
    pub fn count_rpc_uses_of_type(&self, type_simple_name: &str) -> usize {
        let mut count = 0;
        for document in self.get_documents() {
            let content = self.get_content(&document.uri);
            for (req, resp) in document.all_rpc_signatures(content.as_bytes()) {
                if trailing_segment(&req) == type_simple_name
                    || trailing_segment(&resp) == type_simple_name
                {
                    count += 1;
                }
            }
        }
        count
    }

    /// Compute every rename operation that should run for a user's rename
    /// invocation. The first op is the user's *primary* rename (always
    /// present); any additional ops are rpc/request/response chain siblings.
    ///
    /// `decl_uri`/`decl_pos` must already point at the *declaration* of the
    /// symbol being renamed — the caller is responsible for pivoting from a
    /// reference site to the declaration before calling this.
    ///
    /// `chain_rpc_request_response` gates the rpc/request/response chain: when
    /// `false`, only the primary op is returned. It is wired to the
    /// `[config.rename]` `chain_rpc_request_response` setting.
    pub fn compute_rename_ops(
        &self,
        decl_uri: &Url,
        decl_pos: Position,
        new_name: &str,
        ipath: &[PathBuf],
        chain_rpc_request_response: bool,
    ) -> Vec<RenameOp> {
        let mut ops = vec![RenameOp {
            uri: decl_uri.clone(),
            pos: decl_pos,
            new_name: new_name.to_owned(),
        }];
        if chain_rpc_request_response {
            ops.extend(self.compute_chain_siblings(decl_uri, decl_pos, new_name, ipath));
        }
        ops
    }

    /// Apply a sequence of rename ops, merging their per-file edits into a
    /// single map. Returns `None` if the *primary* (first) op fails — in that
    /// case the user's invocation should produce no edit at all. Sibling
    /// failures are silently skipped so the primary always lands.
    pub fn apply_rename_ops(&mut self, ops: &[RenameOp]) -> Option<HashMap<Url, Vec<TextEdit>>> {
        let mut all: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        for (i, op) in ops.iter().enumerate() {
            match self.run_single_rename(op) {
                Some(edits) => {
                    for (u, e) in edits {
                        all.entry(u).or_default().extend(e);
                    }
                }
                None if i == 0 => return None,
                None => {}
            }
        }
        Some(all)
    }

    fn run_single_rename(&mut self, op: &RenameOp) -> Option<BTreeMap<Url, Vec<TextEdit>>> {
        // The workspace is already fully indexed once at startup (see the LSP
        // `initialize` handler), so cross-file rename resolves against the
        // cached metamodel pool without any per-request re-scan.
        let target_fqn = self.resolve_target_fqn(&op.uri, op.pos)?;
        Some(self.rename_for_fqn(&target_fqn, &op.new_name))
    }

    fn compute_chain_siblings(
        &self,
        decl_uri: &Url,
        decl_pos: Position,
        new_name: &str,
        ipath: &[PathBuf],
    ) -> Vec<RenameOp> {
        if new_name.is_empty() {
            return vec![];
        }
        let Some(document) = self.get_document(decl_uri) else {
            return vec![];
        };
        let content = self.get_content(decl_uri);
        let bytes = content.as_bytes();

        if document.rpc_at_position(decl_pos, bytes).is_some() {
            return self.chain_from_rpc_cursor(decl_uri, decl_pos, new_name, ipath);
        }
        if document.message_name_at_position(decl_pos, bytes).is_some() {
            return self.chain_from_message_cursor(decl_uri, decl_pos, new_name, ipath);
        }
        vec![]
    }

    /// Case A: user invoked rename on an `rpc_name`. The primary is the rpc
    /// rename; siblings are the convention-matching request/response messages.
    fn chain_from_rpc_cursor(
        &self,
        decl_uri: &Url,
        decl_pos: Position,
        new_name: &str,
        ipath: &[PathBuf],
    ) -> Vec<RenameOp> {
        let document = self.get_document(decl_uri).expect("checked by caller");
        let content = self.get_content(decl_uri);
        let (old_rpc_name, request_text, response_text) = document
            .rpc_at_position(decl_pos, content.as_bytes())
            .expect("checked by caller");
        self.sibling_message_ops(
            decl_uri,
            &old_rpc_name,
            new_name,
            ipath,
            &[
                ("Request", request_text.as_str()),
                ("Response", response_text.as_str()),
            ],
        )
    }

    /// Case B: user invoked rename on a `message_name` matching the
    /// `<Rpc>Request` / `<Rpc>Response` convention. Primary is the message
    /// rename; we additionally rename the rpc and the opposite sibling.
    ///
    /// To guard against unrelated rpcs in other packages that happen to share
    /// the same simple name, we enumerate *all* candidate rpcs and pick the
    /// unique one whose request/response slot resolves (via the workspace's
    /// name-resolution rules) to the user's primary message. If zero or more
    /// than one rpc matches, the chain is silently dropped.
    fn chain_from_message_cursor(
        &self,
        decl_uri: &Url,
        decl_pos: Position,
        new_name: &str,
        ipath: &[PathBuf],
    ) -> Vec<RenameOp> {
        let document = self.get_document(decl_uri).expect("checked by caller");
        let content = self.get_content(decl_uri);
        let msg_name = document
            .message_name_at_position(decl_pos, content.as_bytes())
            .expect("checked by caller");

        let Some((rpc_base, primary_suffix, new_rpc_base)) =
            strip_convention_suffix(&msg_name, new_name)
        else {
            return vec![];
        };
        if rpc_base.is_empty() || new_rpc_base.is_empty() {
            return vec![];
        }

        // Enumerate rpcs by simple name, then keep only those whose primary
        // slot resolves to the user's actual declaration.
        let mut matching: Vec<(Location, String, String)> = vec![];
        for rpc_loc in self.find_rpc_decls(&rpc_base) {
            let Some(rpc_document) = self.get_document(&rpc_loc.uri) else {
                continue;
            };
            let rpc_content = self.get_content(&rpc_loc.uri);
            let Some((_, rpc_req, rpc_resp)) =
                rpc_document.rpc_at_position(rpc_loc.range.start, rpc_content.as_bytes())
            else {
                continue;
            };
            let slot_text = if primary_suffix == "Request" {
                &rpc_req
            } else {
                &rpc_resp
            };
            let rpc_pkg = rpc_document.package_name();
            let resolves_to_primary = self
                .resolve_identifier_locations(rpc_pkg, slot_text)
                .iter()
                .any(|l| l.uri == *decl_uri && is_position_inside_range(decl_pos, l.range));
            if resolves_to_primary {
                matching.push((rpc_loc, rpc_req, rpc_resp));
            }
        }
        if matching.len() != 1 {
            return vec![];
        }
        let (rpc_loc, rpc_req, rpc_resp) = matching.into_iter().next().unwrap();

        // Uniqueness: only chain if the user's primary message is referenced
        // by exactly one rpc. Otherwise a chained rename would silently break
        // another rpc that shares the type.
        let expected_primary = format!("{rpc_base}{primary_suffix}");
        if self.count_rpc_uses_of_type(&expected_primary) != 1 {
            return vec![];
        }

        let mut ops = vec![RenameOp {
            uri: rpc_loc.uri.clone(),
            pos: rpc_loc.range.start,
            new_name: new_rpc_base.clone(),
        }];
        let (opposite_suffix, opposite_text) = if primary_suffix == "Request" {
            ("Response", rpc_resp.as_str())
        } else {
            ("Request", rpc_req.as_str())
        };
        ops.extend(self.sibling_message_ops(
            &rpc_loc.uri,
            &rpc_base,
            &new_rpc_base,
            ipath,
            &[(opposite_suffix, opposite_text)],
        ));
        ops
    }

    /// Resolve each `(suffix, type_text)` slot to a message declaration and
    /// build the corresponding rename op, gated on convention + uniqueness.
    fn sibling_message_ops(
        &self,
        anchor_uri: &Url,
        old_rpc_name: &str,
        new_rpc_name: &str,
        _ipath: &[PathBuf],
        slots: &[(&str, &str)],
    ) -> Vec<RenameOp> {
        let Some(anchor_document) = self.get_document(anchor_uri) else {
            return vec![];
        };
        let anchor_package = anchor_document.package_name().to_owned();

        let mut ops = vec![];
        for (suffix, type_text) in slots {
            let expected_name = format!("{old_rpc_name}{suffix}");
            if trailing_segment(type_text) != expected_name {
                continue;
            }
            if self.count_rpc_uses_of_type(&expected_name) != 1 {
                continue;
            }
            let locations = self.resolve_identifier_locations(&anchor_package, type_text);
            let Some(decl) = locations.into_iter().next() else {
                continue;
            };
            ops.push(RenameOp {
                uri: decl.uri,
                pos: decl.range.start,
                new_name: format!("{new_rpc_name}{suffix}"),
            });
        }
        ops
    }
}

/// If `msg_name` ends with `Request` or `Response` and `new_name` ends with
/// the same suffix, return `(rpc_base, suffix, new_rpc_base)`. Otherwise
/// `None`. Used to detect when a message rename can plausibly drive a chain.
fn strip_convention_suffix(
    msg_name: &str,
    new_name: &str,
) -> Option<(String, &'static str, String)> {
    for suffix in ["Request", "Response"] {
        if let (Some(base), Some(new_base)) =
            (msg_name.strip_suffix(suffix), new_name.strip_suffix(suffix))
        {
            return Some((base.to_owned(), suffix, new_base.to_owned()));
        }
    }
    None
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use async_lsp::lsp_types::{Position, Url};
    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;
    use crate::state::rename::RenameOp;

    fn make_state(files: &[(&str, &str)], ipath: &[PathBuf]) -> ProtoLanguageState {
        let mut state = ProtoLanguageState::new();
        for (uri, content) in files {
            let parsed_uri = uri.parse().unwrap();
            state.upsert_file(&parsed_uri, content, ipath, 2, &Config::default(), false);
        }
        state
    }

    fn op(uri: &str, line: u32, character: u32, new_name: &str) -> RenameOp {
        RenameOp {
            uri: uri.parse().unwrap(),
            pos: Position { line, character },
            new_name: new_name.to_owned(),
        }
    }

    #[test]
    fn test_rename_for_fqn() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[
                ("file://input/a.proto", include_str!("input/a.proto")),
                ("file://input/b.proto", include_str!("input/b.proto")),
                ("file://input/c.proto", include_str!("input/c.proto")),
            ],
            &ipath,
        );

        // Rename a top-level message: declaration + every reference (including
        // nested-qualified usages like `Author.Address`) are updated.
        assert_yaml_snapshot!(state.rename_for_fqn("com.workspace.Author", "Writer"));
        // Rename a nested message referenced via qualified paths.
        assert_yaml_snapshot!(state.rename_for_fqn("com.workspace.Author.Address", "Location"));
        // Rename a message referenced from another package (fully qualified).
        assert_yaml_snapshot!(state.rename_for_fqn("com.utility.Foobar.Baz", "Baaz"));
    }

    #[test]
    fn test_references_for_fqn() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[
                ("file://input/a.proto", include_str!("input/a.proto")),
                ("file://input/b.proto", include_str!("input/b.proto")),
                ("file://input/c.proto", include_str!("input/c.proto")),
            ],
            &ipath,
        );

        assert_yaml_snapshot!(state.references_for_fqn("com.workspace.Author"));
        assert_yaml_snapshot!(state.references_for_fqn("com.workspace.Author.Address"));
        assert_yaml_snapshot!(state.references_for_fqn("com.utility.Foobar.Baz"));
        assert!(
            state
                .references_for_fqn("com.nonexistent.Missing")
                .is_empty()
        );
    }

    #[test]
    fn test_resolve_target_fqn() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[
                ("file://input/a.proto", include_str!("input/a.proto")),
                ("file://input/b.proto", include_str!("input/b.proto")),
            ],
            &ipath,
        );
        let a_uri: Url = "file://input/a.proto".parse().unwrap();
        let b_uri: Url = "file://input/b.proto".parse().unwrap();

        // Cursor on the `Author` message declaration name in b.proto.
        assert_eq!(
            state.resolve_target_fqn(
                &b_uri,
                Position {
                    line: 5,
                    character: 10
                }
            ),
            Some("com.workspace.Author".to_owned())
        );
        // Cursor on the `Author` type reference inside a field in a.proto.
        assert_eq!(
            state.resolve_target_fqn(
                &a_uri,
                Position {
                    line: 11,
                    character: 5
                }
            ),
            Some("com.workspace.Author".to_owned())
        );
        // Cursor on whitespace -> None.
        assert_eq!(
            state.resolve_target_fqn(
                &a_uri,
                Position {
                    line: 0,
                    character: 0
                }
            ),
            None
        );
    }

    #[test]
    fn test_find_rpc_decls_and_count_uses() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let msg_uri = "file://input/messages.proto".parse().unwrap();
        let svc = include_str!("input/service.proto");
        let msg = include_str!("input/messages.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&svc_uri, svc, &ipath, 2, &Config::default(), false);
        state.upsert_file(&msg_uri, msg, &ipath, 2, &Config::default(), false);

        // Lookup hits the rpc_name node in service.proto.
        let mut locs = state.find_rpc_decls("GetBook");
        assert_eq!(locs.len(), 1, "expected exactly one GetBook rpc");
        let loc = locs.pop().unwrap();
        assert!(loc.uri.as_str().ends_with("service.proto"));
        assert_eq!(loc.range.start.line, 7);

        assert!(state.find_rpc_decls("DoesNotExist").is_empty());

        // Convention-following types are uniquely used.
        assert_eq!(state.count_rpc_uses_of_type("GetBookRequest"), 1);
        assert_eq!(state.count_rpc_uses_of_type("GetBookResponse"), 1);
        // Shared types are seen twice.
        assert_eq!(state.count_rpc_uses_of_type("SharedReq"), 2);
        assert_eq!(state.count_rpc_uses_of_type("SharedResp"), 2);
        // Non-convention type still uniquely used as a response.
        assert_eq!(state.count_rpc_uses_of_type("Bar"), 1);
        // Unrelated names see no uses.
        assert_eq!(state.count_rpc_uses_of_type("Unrelated"), 0);
    }

    #[test]
    fn test_compute_rename_ops_cross_package_collision_blocks_chain() {
        // Two services in different packages each declare `rpc GetBook`. Only
        // pkg foo follows the convention; pkg bar uses unrelated message
        // names. Without the per-candidate full-qualified resolution check,
        // find_rpc_decls' iteration order could let bar's GetBook poison the
        // chain. With the fix, foo's GetBook is uniquely identified by
        // resolving its request slot back to the user's primary message.
        let ipath = vec![PathBuf::from("src/state/input")];
        let foo_uri = "file://input/collision_foo.proto".parse().unwrap();
        let state = make_state(
            &[
                (
                    "file://input/collision_foo.proto",
                    include_str!("input/collision_foo.proto"),
                ),
                (
                    "file://input/collision_bar.proto",
                    include_str!("input/collision_bar.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on foo.GetBookRequest message_name (line 4, col 8..22).
        let pos = Position {
            line: 4,
            character: 12,
        };
        let ops = state.compute_rename_ops(&foo_uri, pos, "FetchBookRequest", &ipath, true);

        // Primary + foo's rpc + foo's response. bar's GetBook is NOT touched.
        assert_eq!(ops.len(), 3, "expected exactly the foo trio, got {ops:?}");
        for op in &ops {
            assert!(
                op.uri.as_str().contains("collision_foo"),
                "chain leaked into bar: {op:?}"
            );
        }
    }

    #[test]
    fn test_compute_rename_ops_chain_from_rpc_cursor() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on the `GetBook` rpc name (line 7, col 8..15).
        let pos = Position {
            line: 7,
            character: 10,
        };
        let ops = state.compute_rename_ops(&svc_uri, pos, "FetchBook", &ipath, true);

        // Primary + Request + Response.
        assert_eq!(
            ops.len(),
            3,
            "expected primary rpc + two sibling ops, got {ops:?}"
        );
        assert_eq!(ops[0], op("file://input/service.proto", 7, 10, "FetchBook"));
        assert_eq!(
            ops[1],
            op("file://input/messages.proto", 4, 8, "FetchBookRequest")
        );
        assert_eq!(
            ops[2],
            op("file://input/messages.proto", 5, 8, "FetchBookResponse")
        );
    }

    #[test]
    fn test_compute_rename_ops_chain_disabled() {
        // Same setup as `chain_from_rpc_cursor`, but with the chain flag off:
        // the rpc/request/response chain is gated behind the `[config.rename]`
        // `chain_rpc_request_response` setting, so only the primary op fires.
        let ipath = vec![PathBuf::from("src/state/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on the `GetBook` rpc name (line 7, col 8..15).
        let pos = Position {
            line: 7,
            character: 10,
        };
        let ops = state.compute_rename_ops(&svc_uri, pos, "FetchBook", &ipath, false);

        assert_eq!(
            ops.len(),
            1,
            "expected primary-only (chain off), got {ops:?}"
        );
        assert_eq!(ops[0], op("file://input/service.proto", 7, 10, "FetchBook"));
    }

    #[test]
    fn test_compute_rename_ops_chain_from_request_cursor() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let msg_uri = "file://input/messages.proto".parse().unwrap();
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on `GetBookRequest` message name (line 4, col 8..22).
        let pos = Position {
            line: 4,
            character: 12,
        };
        let ops = state.compute_rename_ops(&msg_uri, pos, "FetchBookRequest", &ipath, true);

        // Primary (request) + rpc + response.
        assert_eq!(
            ops.len(),
            3,
            "expected primary + rpc + response sibling ops, got {ops:?}"
        );
        assert_eq!(
            ops[0],
            op("file://input/messages.proto", 4, 12, "FetchBookRequest")
        );
        assert_eq!(ops[1], op("file://input/service.proto", 7, 8, "FetchBook"));
        assert_eq!(
            ops[2],
            op("file://input/messages.proto", 5, 8, "FetchBookResponse")
        );
    }

    #[test]
    fn test_compute_rename_ops_shared_request_blocks_chain() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let msg_uri = "file://input/messages.proto".parse().unwrap();
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on `SharedReq` message name (line 10). Both ListA and ListB
        // reference SharedReq — chain must not fire.
        let pos = Position {
            line: 10,
            character: 12,
        };
        let ops = state.compute_rename_ops(&msg_uri, pos, "RenamedReq", &ipath, true);
        assert_eq!(
            ops.len(),
            1,
            "expected primary-only (no chain), got {ops:?}"
        );
        assert_eq!(
            ops[0],
            op("file://input/messages.proto", 10, 12, "RenamedReq")
        );
    }

    #[test]
    fn test_compute_rename_ops_new_name_breaks_convention() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let msg_uri = "file://input/messages.proto".parse().unwrap();
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on GetBookRequest, but new name doesn't preserve the
        // `<Rpc>Request` convention — primary-only.
        let pos = Position {
            line: 4,
            character: 12,
        };
        let ops = state.compute_rename_ops(&msg_uri, pos, "Whatever", &ipath, true);
        assert_eq!(
            ops.len(),
            1,
            "expected primary-only (no chain), got {ops:?}"
        );
        assert_eq!(ops[0], op("file://input/messages.proto", 4, 12, "Whatever"));
    }

    #[test]
    fn test_compute_rename_ops_reference_site_pivot_unsupported() {
        // compute_rename_ops requires the caller to have pivoted to the
        // declaration. As a sanity check, calling it with a cursor on a
        // reference site (not a declaration) yields a primary-only op that
        // would no-op the workspace pass. The LSP layer is responsible for
        // resolving the reference to its declaration *before* calling this.
        let ipath = vec![PathBuf::from("src/state/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on the `GetBookRequest` reference inside the rpc signature
        // (line 7, col 16..30).
        let pos = Position {
            line: 7,
            character: 20,
        };
        let ops = state.compute_rename_ops(&svc_uri, pos, "RenamedRequest", &ipath, true);
        // Single op (the primary at the reference site) — no chain. This
        // documents the contract: the LSP layer must pivot first.
        assert_eq!(ops.len(), 1, "{ops:?}");
    }

    #[test]
    fn test_apply_rename_ops_chain_from_rpc_cursor() {
        // End-to-end snapshot of the full `rpc <Name>(<Name>Request) returns
        // (<Name>Response)` convention chain. Renaming `GetBook` →
        // `FetchBook` should fan out into:
        //   - the rpc_name span in service.proto,
        //   - the request type-reference inside that rpc's signature,
        //   - the response type-reference inside that rpc's signature,
        //   - the `GetBookRequest` message decl in messages.proto,
        //   - the `GetBookResponse` message decl in messages.proto.
        // The snapshot pins both the URIs and the exact edit ranges so any
        // regression in chain detection, edit merging, or workspace-pass
        // resolution will show up here.
        let ipath = vec![PathBuf::from("src/state/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let mut state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        let pos = Position {
            line: 7,
            character: 10,
        };
        let ops = state.compute_rename_ops(&svc_uri, pos, "FetchBook", &ipath, true);
        let edits = state
            .apply_rename_ops(&ops)
            .expect("primary rename should not fail");

        // Sort within each file so the snapshot is order-independent across
        // unrelated implementation changes (e.g. op evaluation order).
        let mut normalized: std::collections::BTreeMap<String, Vec<_>> =
            std::collections::BTreeMap::new();
        for (url, mut v) in edits {
            v.sort_by_key(|e| (e.range.start.line, e.range.start.character));
            normalized.insert(url.to_string(), v);
        }
        assert_yaml_snapshot!(normalized);
    }

    #[test]
    fn test_rename_service_and_rpc() {
        // Renaming a service and an rpc should update their declarations plus
        // any cross-file type references (rpc request/response types stay put).
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        assert_yaml_snapshot!(state.rename_for_fqn("com.workspace.Library", "Catalog"));
        assert_yaml_snapshot!(state.rename_for_fqn("com.workspace.GetBook", "FetchBook"));
    }

    #[test]
    fn test_rename_cross_file_message() {
        // `GetBookRequest` is declared in messages.proto and referenced by the
        // rpc in service.proto; renaming it must update the reference site too.
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        assert_yaml_snapshot!(
            state.rename_for_fqn("com.workspace.GetBookRequest", "FetchBookRequest")
        );
    }

    #[test]
    fn test_rename_field_and_enum_value_single_site() {
        // Fields and enum values are referenced only by their own declaration,
        // so a rename is a single-site edit that must not touch anything else.
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[(
                "file://input/single.proto",
                concat!(
                    "syntax = \"proto3\";\n",
                    "package com.single;\n",
                    "enum Color { RED = 0; GREEN = 1; }\n",
                    "message Book { string title = 1; Color color = 2; }\n",
                ),
            )],
            &ipath,
        );

        assert_yaml_snapshot!(state.rename_for_fqn("com.single.Book.title", "name"));
        assert_yaml_snapshot!(state.rename_for_fqn("com.single.Color.RED", "CRIMSON"));
    }

    #[test]
    fn test_rename_partial_name_safety() {
        // Renaming `Book` must not touch `BookShelf` — only exact FQN matches
        // (and nested-qualified usages) are rewritten.
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[(
                "file://input/partial.proto",
                concat!(
                    "syntax = \"proto3\";\n",
                    "package com.p;\n",
                    "message Book {}\nmessage BookShelf { Book b = 1; }\n",
                ),
            )],
            &ipath,
        );

        let edits = state.rename_for_fqn("com.p.Book", "Novel");
        assert_yaml_snapshot!(edits);

        // BookShelf itself must be untouched.
        let shelf = state
            .get_documents()
            .into_iter()
            .flat_map(|d| d.elements)
            .find(|e| e.kind.fqn() == Some("com.p.BookShelf"));
        assert!(shelf.is_some());
    }

    #[test]
    fn test_rename_cross_package_collision_safety() {
        // `com.foo.GetBookRequest` and the (unrelated) rpc in com.bar share the
        // simple name; renaming the com.foo message must not leak into com.bar.
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[
                (
                    "file://input/foo.proto",
                    include_str!("input/collision_foo.proto"),
                ),
                (
                    "file://input/bar.proto",
                    include_str!("input/collision_bar.proto"),
                ),
            ],
            &ipath,
        );

        let edits = state.rename_for_fqn("com.foo.GetBookRequest", "FetchRequest");
        assert_yaml_snapshot!(edits);

        let bar_uri: Url = "file://input/bar.proto".parse().unwrap();
        let bar_touched = edits.contains_key(&bar_uri);
        assert!(
            !bar_touched,
            "com.bar must not be touched by a com.foo rename"
        );
    }

    #[test]
    fn test_apply_rename_ops_from_reference_site() {
        // End-to-end: invoking rename on a *reference site* (the `GetBookRequest`
        // type inside the rpc signature) pivots to the declaration and renames
        // both the declaration and the reference.
        let ipath = vec![PathBuf::from("src/state/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let mut state = make_state(
            &[
                (
                    "file://input/service.proto",
                    include_str!("input/service.proto"),
                ),
                (
                    "file://input/messages.proto",
                    include_str!("input/messages.proto"),
                ),
            ],
            &ipath,
        );

        // Cursor on `GetBookRequest` inside `rpc GetBook(GetBookRequest) ...`
        // at line 7, character 19.
        let pos = Position {
            line: 7,
            character: 19,
        };
        let ops = state.compute_rename_ops(&svc_uri, pos, "FetchBookRequest", &ipath, false);
        let edits = state.apply_rename_ops(&ops).expect("rename should succeed");
        let mut normalized: std::collections::BTreeMap<String, Vec<_>> =
            std::collections::BTreeMap::new();
        for (url, mut v) in edits {
            v.sort_by_key(|e| (e.range.start.line, e.range.start.character));
            normalized.insert(url.to_string(), v);
        }
        assert_yaml_snapshot!(normalized);
    }

    #[test]
    fn test_rename_unknown_symbol_is_noop() {
        let ipath = vec![PathBuf::from("src/state/input")];
        let state = make_state(
            &[("file://input/a.proto", include_str!("input/a.proto"))],
            &ipath,
        );
        let edits = state.rename_for_fqn("com.workspace.DoesNotExist", "X");
        assert!(edits.is_empty());
    }
}
