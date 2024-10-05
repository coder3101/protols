use std::cell::LazyCell;

use protols_tree_sitter_proto::language;
use tree_sitter::Query;

macro_rules! generate_ts_query {
    ($i: ident, $l:literal) => {
        pub(super) const $i: LazyCell<Query> =
            LazyCell::new(|| Query::new(&language(), $l).unwrap());
    };
}

generate_ts_query!(
    QUERY_PACKAGE_NAME,
    "(package (full_ident (identifier)) @id)"
);

generate_ts_query!(QUERY_IMPORTS, "(import (string) @path)");

generate_ts_query!(
    QUERY_SYMBOLS,
    r#"[
    (message (message_name) @id)
    (enum (enum_name) @id)
] @symbol
"#
);
