use async_lsp::lsp_types::{Position, Range, Url};

use super::ProtoLanguageState;
use crate::config::Config;

pub(super) const DEFINITIONS: &str = r#"syntax = "proto3";
package example.api;
message Outer {
  message Header { string value = 1; }
}
"#;

pub(super) fn range_of(source: &str, needle: &str) -> Range {
    let offset = source.find(needle).unwrap();
    let before = &source[..offset];
    let line = u32::try_from(before.bytes().filter(|&b| b == b'\n').count()).unwrap();
    let column = u32::try_from(before.rsplit('\n').next().unwrap().len()).unwrap();
    Range::new(
        Position::new(line, column),
        Position::new(line, column + u32::try_from(needle.len()).unwrap()),
    )
}

pub(super) fn state_with_usage(usage: &str) -> (ProtoLanguageState, Url, Url) {
    let definitions = Url::parse("file:///navigation/definitions.proto").unwrap();
    let document = Url::parse("file:///navigation/usage.proto").unwrap();
    let mut state = ProtoLanguageState::new();
    state.upsert_file(&definitions, DEFINITIONS, &[], 1, &Config::default(), false);
    state.upsert_file(&document, usage, &[], 1, &Config::default(), false);
    (state, document, definitions)
}
