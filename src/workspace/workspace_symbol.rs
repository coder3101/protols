#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use crate::config::Config;
    use crate::state::ProtoLanguageState;

    #[test]
    fn test_workspace_symbols() {
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

        // Test empty query - should return all symbols
        let all_symbols = state.find_workspace_symbols("");
        assert_yaml_snapshot!("all_symbols", all_symbols);

        // Test query for "author" - should match Author and Address
        let author_symbols = state.find_workspace_symbols("author");
        assert_yaml_snapshot!("author_symbols", author_symbols);

        // Test query for "address" - should match Address
        let address_symbols = state.find_workspace_symbols("address");
        assert_yaml_snapshot!("address_symbols", address_symbols);

        // Test query that should not match anything
        let no_match = state.find_workspace_symbols("nonexistent");
        assert!(no_match.is_empty());
    }
}
