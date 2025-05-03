use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use async_lsp::lsp_types::{MarkupContent, MarkupKind};

use crate::{
    context::hoverable::Hoverables, state::ProtoLanguageState, utils::split_identifier_package,
};

static BUITIN_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        ("int32", include_str!("docs/builtin/int32.md")),
        ("int64", include_str!("docs/builtin/int64.md")),
        ("uint32", include_str!("docs/builtin/uint32.md")),
        ("uint64", include_str!("docs/builtin/uint64.md")),
        ("sint32", include_str!("docs/builtin/sint32.md")),
        ("sint64", include_str!("docs/builtin/sint64.md")),
        ("fixed32", include_str!("docs/builtin/fixed32.md")),
        ("fixed64", include_str!("docs/builtin/fixed64.md")),
        ("sfixed32", include_str!("docs/builtin/sfixed32.md")),
        ("sfixed64", include_str!("docs/builtin/sfixed64.md")),
        ("float", include_str!("docs/builtin/float.md")),
        ("double", include_str!("docs/builtin/double.md")),
        ("string", include_str!("docs/builtin/string.md")),
        ("bytes", include_str!("docs/builtin/bytes.md")),
        ("bool", include_str!("docs/builtin/bool.md")),
        ("default", include_str!("docs/builtin/default.md")),
    ])
});

static WELLKNOWN_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        ("google.protobuf.Any", include_str!("docs/wellknown/Any.md")),
        ("google.protobuf.Api", include_str!("docs/wellknown/Api.md")),
        ("google.protobuf.BoolValue", include_str!("docs/wellknown/BoolValue.md")),
        ("google.protobuf.BytesValue", include_str!("docs/wellknown/BytesValue.md")),
        ("google.protobuf.DoubleValue", include_str!("docs/wellknown/DoubleValue.md")),
        ("google.protobuf.Duration", include_str!("docs/wellknown/Duration.md")),
        ("google.protobuf.Empty", include_str!("docs/wellknown/Empty.md")),
        ("google.protobuf.Enum", include_str!("docs/wellknown/Enum.md")),
        ("google.protobuf.EnumValue", include_str!("docs/wellknown/EnumValue.md")),
        ("google.protobuf.Field", include_str!("docs/wellknown/Field.md")),
        ("google.protobuf.Field.Cardinality", include_str!("docs/wellknown/Field.Cardinality.md")),
        ("google.protobuf.Field.Kind", include_str!("docs/wellknown/Field.Kind.md")),
        ("google.protobuf.FieldMask", include_str!("docs/wellknown/FieldMask.md")),
        ("google.protobuf.FloatValue", include_str!("docs/wellknown/FloatValue.md")),
        ("google.protobuf.Int32Value", include_str!("docs/wellknown/Int32Value.md")),
        ("google.protobuf.Int64Value", include_str!("docs/wellknown/Int64Value.md")),
        ("google.protobuf.ListValue", include_str!("docs/wellknown/ListValue.md")),
        ("google.protobuf.Method", include_str!("docs/wellknown/Method.md")),
        ("google.protobuf.Mixin", include_str!("docs/wellknown/Mixin.md")),
        ("google.protobuf.NullValue", include_str!("docs/wellknown/NullValue.md")),
        ("google.protobuf.Option", include_str!("docs/wellknown/Option.md")),
        ("google.protobuf.SourceContext", include_str!("docs/wellknown/SourceContext.md")),
        ("google.protobuf.StringValue", include_str!("docs/wellknown/StringValue.md")),
        ("google.protobuf.Struct", include_str!("docs/wellknown/Struct.md")),
        ("google.protobuf.Syntax", include_str!("docs/wellknown/Syntax.md")),
        ("google.protobuf.Timestamp", include_str!("docs/wellknown/Timestamp.md")),
        ("google.protobuf.Type", include_str!("docs/wellknown/Type.md")),
        ("google.protobuf.UInt32Value", include_str!("docs/wellknown/UInt32Value.md")),
        ("google.protobuf.UInt64Value", include_str!("docs/wellknown/UInt64Value.md")),
        ("google.protobuf.Value", include_str!("docs/wellknown/Value.md")),
    ])
});

impl ProtoLanguageState {
    pub fn hover(
        &self,
        ipath: &[PathBuf],
        curr_package: &str,
        hv: Hoverables,
    ) -> Option<MarkupContent> {
        let v = match hv {
            Hoverables::FieldType(field) => {
                // Type is a builtin
                match BUITIN_DOCS.get(field.as_str()) {
                    Some(docs) => docs.to_string(),
                    _ => String::new(),
                }
            }
            Hoverables::ImportPath(path) => {
                if let Some(p) = ipath.iter().map(|p| p.join(&path)).find(|p| p.exists()) {
                    format!(
                        r#"Import: `{path}` protobuf file,
---
Included from {}"#,
                        p.to_string_lossy(),
                    )
                } else {
                    String::new()
                }
            }
            Hoverables::Identifier(identifier) => {
                let (mut package, identifier) = split_identifier_package(identifier.as_str());
                if package.is_empty() {
                    package = curr_package;
                }

                // Node is user defined type or well known type
                // If user defined,
                let mut result = WELLKNOWN_DOCS
                    .get(format!("{package}.{identifier}").as_str())
                    .map(|&s| s.to_string())
                    .unwrap_or_default();

                // If no well known was found; try parsing from trees.
                if result.is_empty() {
                    for tree in self.get_trees_for_package(package) {
                        let res = tree.hover(identifier, self.get_content(&tree.uri));

                        if res.is_empty() {
                            continue;
                        }

                        result = format!(
                            r#"`{identifier}` message or enum type, package: `{package}`
---
{}"#,
                            res[0].clone()
                        );
                        break;
                    }
                }

                result
            }
        };

        match v {
            v if v.is_empty() => None,
            v => Some(MarkupContent {
                kind: MarkupKind::Markdown,
                value: v,
            }),
        }
    }
}

#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::context::hoverable::Hoverables;
    use crate::state::ProtoLanguageState;
    #[test]
    fn workspace_test_hover() {
        let ipath = vec![std::env::current_dir().unwrap().join("src/workspace/input")];
        let a_uri = "file://input/a.proto".parse().unwrap();
        let b_uri = "file://input/b.proto".parse().unwrap();
        let c_uri = "file://input/c.proto".parse().unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 3, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);

        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.workspace",
            Hoverables::Identifier("google.protobuf.Any".to_string())
        ));
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.workspace",
            Hoverables::Identifier("Author".to_string())
        ));
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.workspace",
            Hoverables::FieldType("int64".to_string())
        ));
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.workspace",
            Hoverables::Identifier("Author.Address".to_string())
        ));
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.workspace",
            Hoverables::Identifier("com.utility.Foobar.Baz".to_string())
        ));
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.utility",
            Hoverables::Identifier("Baz".to_string())
        ));
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.workspace",
            Hoverables::Identifier("com.inner.Why".to_string())
        ));
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.workspace",
            Hoverables::Identifier("com.super.secret.SomeSecret".to_string())
        ));
    }
}
