use std::path::PathBuf;

use async_lsp::lsp_types::{MarkupContent, MarkupKind};

use crate::{
    context::hoverable::Hoverables, docs, state::ProtoLanguageState,
    utils::split_identifier_package,
};

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
                match docs::BUITIN.get(field.as_str()) {
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
                let mut result = docs::WELLKNOWN
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
