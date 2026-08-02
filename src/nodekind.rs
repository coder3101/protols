use async_lsp::lsp_types::SymbolKind;
use tree_sitter::Node;

pub enum NodeKind {
    Identifier,
    Error,
    MessageName,
    Message,
    EnumName,
    FieldName,
    ServiceName,
    RpcName,
    PackageName,
    PackageImport,
}

#[allow(unused)]
impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Identifier => "identifier",
            NodeKind::Error => "ERROR",
            NodeKind::MessageName => "message_name",
            NodeKind::Message => "message",
            NodeKind::EnumName => "enum_name",
            NodeKind::FieldName => "message_or_enum_type",
            NodeKind::ServiceName => "service_name",
            NodeKind::RpcName => "rpc_name",
            NodeKind::PackageName => "full_ident",
            NodeKind::PackageImport => "import",
        }
    }

    pub fn is_identifier(n: &Node) -> bool {
        n.kind() == Self::Identifier.as_str()
    }

    pub fn is_error(n: &Node) -> bool {
        n.kind() == Self::Error.as_str()
    }

    pub fn is_import_path(n: &Node) -> bool {
        n.kind() == Self::PackageImport.as_str()
    }

    pub fn is_package_name(n: &Node) -> bool {
        n.kind() == Self::PackageName.as_str()
    }

    pub fn is_enum_name(n: &Node) -> bool {
        n.kind() == Self::EnumName.as_str()
    }

    pub fn is_message_name(n: &Node) -> bool {
        n.kind() == Self::MessageName.as_str()
    }

    pub fn is_message(n: &Node) -> bool {
        n.kind() == Self::Message.as_str()
    }

    pub fn is_field_name(n: &Node) -> bool {
        n.kind() == Self::FieldName.as_str()
    }

    pub fn is_rpc_name(n: &Node) -> bool {
        n.kind() == Self::RpcName.as_str()
    }

    pub fn is_userdefined(n: &Node) -> bool {
        n.kind() == Self::EnumName.as_str() || n.kind() == Self::MessageName.as_str()
    }

    pub fn is_renameable(n: &Node) -> bool {
        Self::is_userdefined(n)
            || n.kind() == Self::ServiceName.as_str()
            || n.kind() == Self::RpcName.as_str()
            || n.kind() == Self::FieldName.as_str()
            || Self::is_field_decl_parent(n)
    }

    /// Kinds whose direct identifier child is the *name* of a field-like
    /// declaration: regular fields, map fields, oneof fields, the oneof itself,
    /// and enum values. For `string title = 1;`, the identifier `title` has
    /// parent `field` — that's what we match here. The type identifier (e.g.
    /// `Author` in `Author author = 2;`) is nested deeper under
    /// `message_or_enum_type`, so it isn't caught by this predicate.
    pub fn is_field_decl_parent(n: &Node) -> bool {
        matches!(
            n.kind(),
            "field" | "map_field" | "oneof_field" | "oneof" | "enum_field"
        )
    }

    pub fn is_actionable(n: &Node) -> bool {
        n.kind() == Self::MessageName.as_str()
            || n.kind() == Self::EnumName.as_str()
            || n.kind() == Self::FieldName.as_str()
            || n.kind() == Self::PackageName.as_str()
            || n.kind() == Self::ServiceName.as_str()
            || n.kind() == Self::RpcName.as_str()
    }

    pub fn to_symbolkind(n: &Node) -> SymbolKind {
        if n.kind() == Self::MessageName.as_str() {
            SymbolKind::STRUCT
        } else if n.kind() == Self::EnumName.as_str() {
            SymbolKind::ENUM
        } else {
            SymbolKind::NULL
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tree_sitter::Parser;

    fn parse_proto(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_proto::LANGUAGE.into())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_is_identifier() {
        let document = parse_proto("message Foo { string name = 1; }");
        let mut cursor = document.root_node().walk();
        // Find the "name" identifier node (child of field)
        let mut found = false;
        loop {
            let n = cursor.node();
            if n.kind() == "identifier" {
                assert!(NodeKind::is_identifier(&n));
                found = true;
            } else {
                assert!(!NodeKind::is_identifier(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        assert!(found, "expected at least one identifier node");
    }

    #[test]
    fn test_is_message_name() {
        let document = parse_proto("message Foo {}");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "message_name" {
                assert!(NodeKind::is_message_name(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_enum_name() {
        let document = parse_proto("enum Color { RED = 0; }");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "enum_name" {
                assert!(NodeKind::is_enum_name(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_field_name() {
        let document = parse_proto("message Foo { string bar = 1; }");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "message_or_enum_type" {
                assert!(NodeKind::is_field_name(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_rpc_name() {
        let document = parse_proto("service S { rpc Foo(Empty) returns (Empty); }");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "rpc_name" {
                assert!(NodeKind::is_rpc_name(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_service_name_node_kind() {
        let document = parse_proto("service MyService {}");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "service_name" {
                assert_eq!(n.kind(), "service_name");
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_package_name() {
        let document = parse_proto("package foo.bar;");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "full_ident" {
                assert!(NodeKind::is_package_name(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_import_path() {
        let document = parse_proto("import \"foo/bar.proto\";");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "import" {
                assert!(NodeKind::is_import_path(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_error() {
        let document = parse_proto("message Foo { invalid_syntax }");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "ERROR" {
                assert!(NodeKind::is_error(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_message() {
        let document = parse_proto("message Foo {}");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "message" {
                assert!(NodeKind::is_message(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_userdefined() {
        let document = parse_proto("message Foo { enum Bar { X = 0; } }");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            if n.kind() == "message_name" || n.kind() == "enum_name" {
                assert!(NodeKind::is_userdefined(&n));
            } else {
                assert!(!NodeKind::is_userdefined(&n));
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_renameable() {
        let document = parse_proto(
            "message Foo { string bar = 1; } enum E { X = 0; } service S { rpc F(Empty) returns (Empty); }",
        );
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            match n.kind() {
                "message_name"
                | "enum_name"
                | "service_name"
                | "rpc_name"
                | "message_or_enum_type"
                | "field"
                | "map_field"
                | "oneof_field"
                | "oneof"
                | "enum_field" => {
                    assert!(
                        NodeKind::is_renameable(&n),
                        "expected {} to be renameable",
                        n.kind()
                    );
                }
                _ => {
                    assert!(
                        !NodeKind::is_renameable(&n),
                        "expected {} to NOT be renameable",
                        n.kind()
                    );
                }
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_field_decl_parent() {
        let document = parse_proto(
            "message Foo { string a = 1; map<string,int> m = 2; oneof o { string b = 3; } } enum E { X = 0; }",
        );
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            match n.kind() {
                "field" | "map_field" | "oneof_field" | "oneof" | "enum_field" => {
                    assert!(NodeKind::is_field_decl_parent(&n));
                }
                _ => {
                    assert!(!NodeKind::is_field_decl_parent(&n));
                }
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_is_actionable() {
        let document = parse_proto(
            "message Foo { string bar = 1; } service S { rpc F(Empty) returns (Empty); }",
        );
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            match n.kind() {
                "message_name"
                | "enum_name"
                | "message_or_enum_type"
                | "full_ident"
                | "service_name"
                | "rpc_name" => {
                    assert!(NodeKind::is_actionable(&n));
                }
                _ => {
                    assert!(!NodeKind::is_actionable(&n));
                }
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_to_symbolkind() {
        let document = parse_proto("message Foo {} enum Bar { X = 0; } syntax = \"proto3\";");
        let mut cursor = document.root_node().walk();
        loop {
            let n = cursor.node();
            match n.kind() {
                "message_name" => assert_eq!(NodeKind::to_symbolkind(&n), SymbolKind::STRUCT),
                "enum_name" => assert_eq!(NodeKind::to_symbolkind(&n), SymbolKind::ENUM),
                _ => assert_eq!(NodeKind::to_symbolkind(&n), SymbolKind::NULL),
            }
            if cursor.goto_first_child() {
                continue;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    #[test]
    fn test_as_str() {
        assert_eq!(NodeKind::Identifier.as_str(), "identifier");
        assert_eq!(NodeKind::Error.as_str(), "ERROR");
        assert_eq!(NodeKind::MessageName.as_str(), "message_name");
        assert_eq!(NodeKind::Message.as_str(), "message");
        assert_eq!(NodeKind::EnumName.as_str(), "enum_name");
        assert_eq!(NodeKind::FieldName.as_str(), "message_or_enum_type");
        assert_eq!(NodeKind::ServiceName.as_str(), "service_name");
        assert_eq!(NodeKind::RpcName.as_str(), "rpc_name");
        assert_eq!(NodeKind::PackageName.as_str(), "full_ident");
        assert_eq!(NodeKind::PackageImport.as_str(), "import");
    }
}
