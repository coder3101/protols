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
        }
    }

    pub fn is_identifier(n: &Node) -> bool {
        n.kind() == Self::Identifier.as_str()
    }

    pub fn is_error(n: &Node) -> bool {
        n.kind() == Self::Error.as_str()
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
