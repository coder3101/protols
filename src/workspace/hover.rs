use std::{collections::HashMap, sync::LazyLock};

use async_lsp::lsp_types::MarkedString;

use crate::{state::ProtoLanguageState, utils::split_identifier_package};

static BUITIN_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        (
            "int32",
            r#"A 32-bit integer (varint encoding)

Values of this type range between `-2147483648` and `2147483647`.
Beware that negative values are encoded as five bytes on the wire!"#,
        ),
        (
            "int64",
            r#"A 64-bit integer (varint encoding)

Values of this type range between `-9223372036854775808` and `9223372036854775807`.
Beware that negative values are encoded as ten bytes on the wire!"#,
        ),
        (
            "uint32",
            r#"A 32-bit unsigned integer (varint encoding)

Values of this type range between `0` and `4294967295`."#,
        ),
        (
            "uint64",
            r#"A 64-bit unsigned integer (varint encoding)

Values of this type range between `0` and `18446744073709551615`."#,
        ),
        (
            "sint32",
            r#"A 32-bit integer (ZigZag encoding)

Values of this type range between `-2147483648` and `2147483647`."#,
        ),
        (
            "sint64",
            r#"A 64-bit integer (ZigZag encoding)

Values of this type range between `-9223372036854775808` and `9223372036854775807`."#,
        ),
        (
            "fixed32",
            r#"A 32-bit unsigned integer (4-byte encoding)

Values of this type range between `0` and `4294967295`."#,
        ),
        (
            "fixed64",
            r#"A 64-bit unsigned integer (8-byte encoding)

Values of this type range between `0` and `18446744073709551615`."#,
        ),
        (
            "sfixed32",
            r#"A 32-bit integer (4-byte encoding)

Values of this type range between `-2147483648` and `2147483647`."#,
        ),
        (
            "sfixed64",
            r#"A 64-bit integer (8-byte encoding)

Values of this type range between `-9223372036854775808` and `9223372036854775807`."#,
        ),
        (
            "float",
            "A single-precision floating point number (IEEE-745.2008 binary32).",
        ),
        (
            "double",
            "A double-precision floating point number (IEEE-745.2008 binary64).",
        ),
        (
            "string",
            r#"A string of text.

Stores at most 4GB of text. Intended to be UTF-8 encoded Unicode; use `bytes` if you need other encodings."#,
        ),
        (
            "bytes",
            r#"A blob of arbitrary bytes.

Stores at most 4GB of binary data. Encoded as base64 in JSON."#,
        ),
        (
            "bool",
            r#"A Boolean value: `true` or `false`.

Encoded as a single byte: `0x00` or `0xff` (all non-zero bytes decode to `true`)."#,
        ),
        (
            "default",
            r#"A magic option that specifies the field's default value.

Unlike every other option on a field, this does not have a corresponding field in
`google.protobuf.FieldOptions`; it is implemented by compiler magic."#,
        ),
    ])
});

impl ProtoLanguageState {
    pub fn hover(&self, curr_package: &str, identifier: &str) -> Vec<MarkedString> {
        if let Some(docs) = BUITIN_DOCS.get(identifier) {
            return vec![MarkedString::String(docs.to_string())];
        }

        let (mut package, identifier) = split_identifier_package(identifier);
        if package.is_empty() {
            package = curr_package;
        }

        self.get_trees_for_package(package)
            .into_iter()
            .fold(vec![], |mut v, tree| {
                v.extend(tree.hover(identifier, self.get_content(&tree.uri)));
                v
            })
    }
}

#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use crate::state::ProtoLanguageState;

    #[test]
    fn workspace_test_hover() {
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned());
        state.upsert_file(&b_uri, b.to_owned());
        state.upsert_file(&c_uri, c.to_owned());

        assert_yaml_snapshot!(state.hover("com.workspace", "Author"));
        assert_yaml_snapshot!(state.hover("com.workspace", "int64"));
        assert_yaml_snapshot!(state.hover("com.workspace", "Author.Address"));
        assert_yaml_snapshot!(state.hover("com.workspace", "com.utility.Foobar.Baz"));
        assert_yaml_snapshot!(state.hover("com.utility", "Baz"));
    }
}
