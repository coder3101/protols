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
