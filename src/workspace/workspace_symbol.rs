#[cfg(test)]
mod test {
    use async_lsp::lsp_types::Url;
    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;

    #[test]
    fn test_workspace_symbols() {
        let current_dir = std::env::current_dir().unwrap();
        let ipath = vec![current_dir.join("src/workspace/input")];
        let base_uri_str = Url::from_directory_path(&current_dir)
            .unwrap()
            .to_string()
            .trim_end_matches('/')
            .to_string();
        let a_uri = Url::from_file_path(current_dir.join("src/workspace/input/a.proto")).unwrap();
        let b_uri = Url::from_file_path(current_dir.join("src/workspace/input/b.proto")).unwrap();
        let c_uri = Url::from_file_path(current_dir.join("src/workspace/input/c.proto")).unwrap();

        let a = include_str!("input/a.proto");
        let b = include_str!("input/b.proto");
        let c = include_str!("input/c.proto");

        let mut state: ProtoLanguageState = ProtoLanguageState::new();
        state.upsert_file(&a_uri, a.to_owned(), &ipath, 3, &Config::default(), false);
        state.upsert_file(&b_uri, b.to_owned(), &ipath, 2, &Config::default(), false);
        state.upsert_file(&c_uri, c.to_owned(), &ipath, 2, &Config::default(), false);

        // Test empty query - should return all symbols
        let all_symbols = state.find_workspace_symbols("");
        let base_uri_1 = base_uri_str.clone();
        assert_yaml_snapshot!(all_symbols, { "[].location.uri" => insta::dynamic_redaction(move |c, _| {
            let uri_str = c.as_str().unwrap();

            assert!(
                uri_str.contains(&base_uri_1),
                "URI {} should contain {}", uri_str, base_uri_1
            );

            let file_name = uri_str.split('/').next_back().unwrap();
            format!("file://<redacted>/src/workspace/input/{}", file_name)

        })});

        // Test query for "author" - should match Author and Address
        let author_symbols = state.find_workspace_symbols("author");
        let base_uri_2 = base_uri_str.clone();
        assert_yaml_snapshot!(author_symbols, {"[].location.uri" => insta::dynamic_redaction(move |c ,_|{
            let uri_str = c.as_str().unwrap();

            assert!(
                uri_str.contains(&base_uri_2),
                "URI {} should contain {}", uri_str, base_uri_2
            );

            let file_name = uri_str.split('/').next_back().unwrap();
            format!("file://<redacted>/src/workspace/input/{}", file_name)
        })});

        // Test query for "address" - should match Address
        let address_symbols = state.find_workspace_symbols("address");
        let base_uri_3 = base_uri_str.clone();
        assert_yaml_snapshot!(address_symbols, {"[].location.uri" => insta::dynamic_redaction(move |c ,_|{
            let uri_str = c.as_str().unwrap();

            assert!(
                uri_str.contains(&base_uri_3),
                "URI {} should contain {}", uri_str, base_uri_3
            );


            let file_name = uri_str.split('/').next_back().unwrap();
            format!("file://<redacted>/src/workspace/input/{}", file_name)
        })});

        // Test query that should not match anything
        let no_match = state.find_workspace_symbols("nonexistent");
        assert!(no_match.is_empty());
    }
}
