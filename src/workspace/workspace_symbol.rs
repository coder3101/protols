#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;
    use insta::internals::{Content, ContentPath};

    use crate::config::Config;
    use crate::state::ProtoLanguageState;

    #[test]
    fn test_workspace_symbols() {
        let current_dir = std::env::current_dir().unwrap();
        let ipath = vec![current_dir.join("src/workspace/input")];
        let a_uri = (&format!(
            "file://{}/src/workspace/input/a.proto",
            current_dir.to_str().unwrap()
        ))
            .parse()
            .unwrap();
        let b_uri = (&format!(
            "file://{}/src/workspace/input/b.proto",
            current_dir.to_str().unwrap()
        ))
            .parse()
            .unwrap();
        let c_uri = (&format!(
            "file://{}/src/workspace/input/c.proto",
            current_dir.to_str().unwrap()
        ))
            .parse()
            .unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 3, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);

        // Test empty query - should return all symbols
        let all_symbols = state.find_workspace_symbols("");
        let cdir = current_dir.to_str().unwrap().to_string();
        assert_yaml_snapshot!(all_symbols, { "[].location.uri" => insta::dynamic_redaction(move |c, _| {
            assert!(
                c.as_str()
                    .unwrap()
                    .contains(&cdir)
            );
            format!(
                "file://{}/src/workspace/input/{}",
                "<redacted>",
                c.as_str().unwrap().split('/').last().unwrap()
            )

        })});

        // Test query for "author" - should match Author and Address
        let author_symbols = state.find_workspace_symbols("author");
        let cdir = current_dir.to_str().unwrap().to_string();
        assert_yaml_snapshot!(author_symbols, {"[].location.uri" => insta::dynamic_redaction(move |c ,_|{
            assert!(
                c.as_str()
                    .unwrap()
                    .contains(&cdir)
            );
            format!(
                "file://{}/src/workspace/input/{}",
                "<redacted>",
                c.as_str().unwrap().split('/').last().unwrap()
            )
        })});

        // Test query for "address" - should match Address
        let address_symbols = state.find_workspace_symbols("address");
        assert_yaml_snapshot!(address_symbols, {"[].location.uri" => insta::dynamic_redaction(move |c ,_|{
            assert!(
                c.as_str()
                    .unwrap()
                    .contains(&current_dir.to_str().unwrap())
            );
            format!(
                "file://{}/src/workspace/input/{}",
                "<redacted>",
                c.as_str().unwrap().split('/').last().unwrap()
            )
        })});

        // Test query that should not match anything
        let no_match = state.find_workspace_symbols("nonexistent");
        assert!(no_match.is_empty());
    }
}
