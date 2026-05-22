use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use async_lsp::lsp_types::{Location, Position, ProgressParamsValue, Range, TextEdit, Url};

use crate::context::jumpable::Jumpable;
use crate::nodekind::NodeKind;
use crate::state::ProtoLanguageState;
use crate::utils::{split_identifier_package, trailing_segment, ts_to_lsp_position};

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
    pub fn rename_fields(
        &mut self,
        current_package: &str,
        identifier: &str,
        new_text: &str,
        workspace: PathBuf,
        progress_sender: Option<Sender<ProgressParamsValue>>,
    ) -> HashMap<Url, Vec<TextEdit>> {
        self.parse_all_from_workspace(workspace, progress_sender);
        let (_, identifier) = split_identifier_package(identifier);
        self.get_trees()
            .into_iter()
            .fold(HashMap::new(), |mut h, tree| {
                let content = self.get_content(&tree.uri);
                let package = tree.get_package_name(content.as_ref()).unwrap_or(".");
                let mut old = identifier.to_string();
                let mut new = new_text.to_string();
                let mut v = vec![];

                // Global scope: Reference by only . or within global directly
                if current_package == "." {
                    if package == "." {
                        v.extend(tree.rename_field(&old, &new, content.as_str()));
                    }

                    old = format!(".{old}");
                    new = format!(".{new}");

                    v.extend(tree.rename_field(&old, &new, content.as_str()));

                    if !v.is_empty() {
                        h.insert(tree.uri.clone(), v);
                    }
                    return h;
                }

                let full_old = format!("{current_package}.{old}");
                let full_new = format!("{current_package}.{new}");
                let global_full_old = format!(".{current_package}.{old}");
                let global_full_new = format!(".{current_package}.{new}");

                // Current package: Reference by full or relative name or directly
                if current_package == package {
                    v.extend(tree.rename_field(&old, &new, content.as_str()));
                } else if current_package.starts_with(package) {
                    // Safety: prefix check already done
                    // get the relative part of the package
                    let packagepart = current_package
                        .strip_prefix(package)
                        .unwrap()
                        .trim_start_matches('.');
                    let relative_old = format!("{packagepart}.{old}");
                    let relative_new = format!("{packagepart}.{new}");
                    v.extend(tree.rename_field(&relative_old, &relative_new, content.as_str()));
                }

                // Otherwise, full reference
                v.extend(tree.rename_field(&full_old, &full_new, content.as_str()));
                v.extend(tree.rename_field(&global_full_old, &global_full_new, content.as_str()));

                if !v.is_empty() {
                    h.insert(tree.uri.clone(), v);
                }
                h
            })
    }

    pub fn reference_fields(
        &mut self,
        current_package: &str,
        identifier: &str,
        workspace: PathBuf,
        progress_sender: Option<Sender<ProgressParamsValue>>,
    ) -> Option<Vec<Location>> {
        self.parse_all_from_workspace(workspace, progress_sender);
        let (_, identifier) = split_identifier_package(identifier);
        let r = self
            .get_trees()
            .into_iter()
            .fold(Vec::<Location>::new(), |mut v, tree| {
                let content = self.get_content(&tree.uri);
                let package = tree.get_package_name(content.as_ref()).unwrap_or(".");
                let mut ident = identifier.to_owned();
                // Global scope: Reference by only . or within global directly
                if current_package == "." {
                    if package == "." {
                        v.extend(tree.reference_field(&ident, content.as_str()));
                    }

                    ident = format!(".{ident}");
                    v.extend(tree.reference_field(&ident, content.as_str()));

                    return v;
                }

                let full_ident = format!("{current_package}.{ident}");
                let global_full_ident = format!(".{current_package}.{ident}");

                // Current package: Reference by full or relative name or directly
                if current_package == package {
                    v.extend(tree.reference_field(&ident, content.as_str()));
                } else if current_package.starts_with(package) {
                    // Safety: prefix check already done
                    // get the relative part of the package
                    let packagepart = current_package
                        .strip_prefix(package)
                        .unwrap()
                        .trim_start_matches('.');
                    let relative = format!("{packagepart}.{ident}");
                    v.extend(tree.reference_field(&relative, content.as_str()));
                }

                // Otherwise, full reference
                v.extend(tree.reference_field(&full_ident, content.as_str()));
                v.extend(tree.reference_field(&global_full_ident, content.as_str()));
                v
            });
        if r.is_empty() { None } else { Some(r) }
    }

    /// Find every rpc declaration in the workspace whose simple name matches
    /// `rpc_name`. Used by the rpc/request/response chain rename to enumerate
    /// candidate rpcs when the user invokes rename on a convention-named
    /// message; the caller then narrows the list by checking which candidate
    /// actually references the user's primary message.
    pub fn find_rpc_decls(&self, rpc_name: &str) -> Vec<Location> {
        let mut out = vec![];
        for tree in self.get_trees() {
            let content = self.get_content(&tree.uri);
            for node in tree.find_all_nodes(NodeKind::is_rpc_name) {
                let Ok(text) = node.utf8_text(content.as_bytes()) else {
                    continue;
                };
                if text == rpc_name {
                    out.push(Location {
                        uri: tree.uri.clone(),
                        range: Range {
                            start: ts_to_lsp_position(&node.start_position()),
                            end: ts_to_lsp_position(&node.end_position()),
                        },
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
        for tree in self.get_trees() {
            let content = self.get_content(&tree.uri);
            for (req, resp) in tree.all_rpc_signatures(content.as_bytes()) {
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
    pub fn apply_rename_ops(
        &mut self,
        ops: &[RenameOp],
        workspace: PathBuf,
        progress_sender: Option<Sender<ProgressParamsValue>>,
    ) -> Option<HashMap<Url, Vec<TextEdit>>> {
        let mut all: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        let mut progress = progress_sender;
        for (i, op) in ops.iter().enumerate() {
            // Only the first op gets the progress sender; subsequent ops would
            // double-report.
            let sender = progress.take();
            match self.run_single_rename(op, workspace.clone(), sender) {
                Some(edits) => {
                    for (u, e) in edits {
                        all.entry(u).or_default().extend(e);
                    }
                }
                None if i == 0 => return None,
                None => continue,
            }
        }
        Some(all)
    }

    fn run_single_rename(
        &mut self,
        op: &RenameOp,
        workspace: PathBuf,
        progress_sender: Option<Sender<ProgressParamsValue>>,
    ) -> Option<HashMap<Url, Vec<TextEdit>>> {
        let tree = self.get_tree(&op.uri)?;
        let content = self.get_content(&op.uri);
        let package = tree.get_package_name(content.as_bytes()).unwrap_or(".");

        let (edit, otext, ntext) = tree.rename_tree(&op.pos, &op.new_name, content.as_bytes())?;

        let mut h: HashMap<Url, Vec<TextEdit>> = HashMap::new();
        h.extend(self.rename_fields(package, &otext, &ntext, workspace, progress_sender));
        h.entry(tree.uri.clone()).or_default().extend(edit);
        Some(h)
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
        let Some(tree) = self.get_tree(decl_uri) else {
            return vec![];
        };
        let content = self.get_content(decl_uri);
        let bytes = content.as_bytes();

        if tree.rpc_at_position(&decl_pos, bytes).is_some() {
            return self.chain_from_rpc_cursor(decl_uri, decl_pos, new_name, ipath);
        }
        if tree.message_name_at_position(&decl_pos, bytes).is_some() {
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
        let tree = self.get_tree(decl_uri).expect("checked by caller");
        let content = self.get_content(decl_uri);
        let (old_rpc_name, request_text, response_text) = tree
            .rpc_at_position(&decl_pos, content.as_bytes())
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
        let tree = self.get_tree(decl_uri).expect("checked by caller");
        let content = self.get_content(decl_uri);
        let msg_name = tree
            .message_name_at_position(&decl_pos, content.as_bytes())
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
            let Some(rpc_tree) = self.get_tree(&rpc_loc.uri) else {
                continue;
            };
            let rpc_content = self.get_content(&rpc_loc.uri);
            let Some((_, rpc_req, rpc_resp)) =
                rpc_tree.rpc_at_position(&rpc_loc.range.start, rpc_content.as_bytes())
            else {
                continue;
            };
            let slot_text = if primary_suffix == "Request" {
                &rpc_req
            } else {
                &rpc_resp
            };
            let rpc_pkg = rpc_tree
                .get_package_name(rpc_content.as_bytes())
                .unwrap_or(".");
            let resolves_to_primary = self
                .definition(ipath, rpc_pkg, Jumpable::Identifier(slot_text.clone()))
                .iter()
                .any(|l| l.uri == *decl_uri && position_in_range(decl_pos, l.range));
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
        ipath: &[PathBuf],
        slots: &[(&str, &str)],
    ) -> Vec<RenameOp> {
        let Some(anchor_tree) = self.get_tree(anchor_uri) else {
            return vec![];
        };
        let anchor_content = self.get_content(anchor_uri);
        let anchor_package = anchor_tree
            .get_package_name(anchor_content.as_bytes())
            .unwrap_or(".")
            .to_owned();

        let mut ops = vec![];
        for (suffix, type_text) in slots {
            let expected_name = format!("{old_rpc_name}{suffix}");
            if trailing_segment(type_text) != expected_name {
                continue;
            }
            if self.count_rpc_uses_of_type(&expected_name) != 1 {
                continue;
            }
            let locations = self.definition(
                ipath,
                &anchor_package,
                Jumpable::Identifier((*type_text).to_owned()),
            );
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

/// True iff `pos` falls within `range` (inclusive of both endpoints). Used to
/// match a possibly mid-identifier cursor against a definition's full-span
/// range when verifying that two locations refer to the same symbol.
fn position_in_range(pos: Position, range: Range) -> bool {
    (pos.line, pos.character) >= (range.start.line, range.start.character)
        && (pos.line, pos.character) <= (range.end.line, range.end.character)
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

    use async_lsp::lsp_types::Position;
    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;
    use crate::workspace::rename::RenameOp;

    fn make_state(files: &[(&str, &str)], ipath: &[PathBuf]) -> ProtoLanguageState {
        let mut state = ProtoLanguageState::new();
        for (uri, content) in files {
            let parsed_uri = uri.parse().unwrap();
            state.upsert_file(
                &parsed_uri,
                (*content).to_owned(),
                ipath,
                2,
                &Config::default(),
                false,
            );
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
    fn test_rename() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);

        assert_yaml_snapshot!(state.rename_fields(
            "com.workspace",
            "Author",
            "Writer",
            PathBuf::from("src/workspace/input"),
            None
        ));
        assert_yaml_snapshot!(state.rename_fields(
            "com.workspace",
            "Author.Address",
            "Author.Location",
            PathBuf::from("src/workspace/input"),
            None
        ));
        assert_yaml_snapshot!(state.rename_fields(
            "com.utility",
            "Foobar.Baz",
            "Foobar.Baaz",
            PathBuf::from("src/workspace/input"),
            None
        ));
    }

    #[test]
    fn test_reference() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);

        assert_yaml_snapshot!(state.reference_fields(
            "com.workspace",
            "Author",
            PathBuf::from("src/workspace/input"),
            None
        ));
        assert_yaml_snapshot!(state.reference_fields(
            "com.workspace",
            "Author.Address",
            PathBuf::from("src/workspace/input"),
            None
        ));
    }

    #[test]
    fn test_find_rpc_decls_and_count_uses() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
        let svc_uri = "file://input/service.proto".parse().unwrap();
        let msg_uri = "file://input/messages.proto".parse().unwrap();
        let svc = include_str!("input/service.proto");
        let msg = include_str!("input/messages.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(
            &svc_uri,
            svc.to_owned(),
            &ipath,
            2,
            &Config::default(),
            false,
        );
        state.upsert_file(
            &msg_uri,
            msg.to_owned(),
            &ipath,
            2,
            &Config::default(),
            false,
        );

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
        let ipath = vec![PathBuf::from("src/workspace/input")];
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
        let ipath = vec![PathBuf::from("src/workspace/input")];
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
        let ipath = vec![PathBuf::from("src/workspace/input")];
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

        assert_eq!(ops.len(), 1, "expected primary-only (chain off), got {ops:?}");
        assert_eq!(ops[0], op("file://input/service.proto", 7, 10, "FetchBook"));
    }

    #[test]
    fn test_compute_rename_ops_chain_from_request_cursor() {
        let ipath = vec![PathBuf::from("src/workspace/input")];
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
        let ipath = vec![PathBuf::from("src/workspace/input")];
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
        let ipath = vec![PathBuf::from("src/workspace/input")];
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
        let ipath = vec![PathBuf::from("src/workspace/input")];
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
        let ipath = vec![PathBuf::from("src/workspace/input")];
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
            .apply_rename_ops(&ops, PathBuf::from("src/workspace/input"), None)
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
}
