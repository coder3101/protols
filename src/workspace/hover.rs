use std::path::{Path, PathBuf};

use async_lsp::lsp_types::{MarkupContent, MarkupKind};

use crate::{
    context::hoverable::Hoverables, docs, state::ProtoLanguageState,
    utils::split_identifier_package,
};

fn format_import_path_hover_text(path: &str, p: &Path) -> String {
    format!(
        r#"Import: `{path}` protobuf file,
---
Included from {}"#,
        p.to_string_lossy()
    )
}

fn format_identifier_hover_text(identifier: &str, package: &str, result: &str) -> String {
    format!(
        r#"`{identifier}` message or enum type, package: `{package}`
---
{result}"#
    )
}

impl ProtoLanguageState {
    pub fn hover(
        &self,
        ipath: &[PathBuf],
        curr_package: &str,
        hv: Hoverables,
    ) -> Option<MarkupContent> {
        let v = match hv {
            Hoverables::FieldType(field) => docs::BUITIN
                .get(field.as_str())
                .map(ToString::to_string)
                .unwrap_or_default(),

            Hoverables::ImportPath(path) => ipath
                .iter()
                .map(|p| p.join(&path))
                .find(|p| p.exists())
                .map(|p| format_import_path_hover_text(&path, &p))
                .unwrap_or_default(),

            Hoverables::Identifier(identifier) => {
                let (mut package, identifier) = split_identifier_package(identifier.as_str());
                if package.is_empty() {
                    package = curr_package;
                }

                // Identifier is user defined type or well known type

                // If well known types, check in wellknown docs,
                // otherwise check in trees
                match docs::WELLKNOWN
                    .get(format!("{package}.{identifier}").as_str())
                    .map(|&s| s.to_string())
                {
                    Some(res) => res,
                    None => {
                        let mut trees = vec![];

                        // If package != curr_package, either identifier is from a completely new package
                        // or relative package from within. As per name resolution first resolve relative
                        // packages, add all relative trees in search list
                        if curr_package != package {
                            let fullpackage = format!("{curr_package}.{package}");
                            trees.append(&mut self.get_trees_for_package(&fullpackage));
                        }

                        // Add all direct package trees
                        trees.append(&mut self.get_trees_for_package(package));

                        // Find the first field hovered in the trees
                        let res = trees.iter().find_map(|tree| {
                            let content = self.get_content(&tree.uri);
                            let res = tree.hover(identifier, content);
                            if res.is_empty() {
                                None
                            } else {
                                Some(res[0].clone())
                            }
                        });

                        // Format the hover text and return
                        // TODO: package here is literally what was hovered, incase of
                        // relative it is only the relative part, should be full path, should
                        // probably figure out the package from the tree which provides hover and
                        // pass here.
                        res.map(|r| format_identifier_hover_text(identifier, package, &r))
                            .unwrap_or_default()
                    }
                }
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
            "com.inner",
            Hoverables::Identifier(".com.inner.secret.SomeSecret".to_string())
        ));
        // relative path hover
        assert_yaml_snapshot!(state.hover(
            &ipath,
            "com.inner",
            Hoverables::Identifier("secret.SomeSecret".to_string())
        ))
    }
}
