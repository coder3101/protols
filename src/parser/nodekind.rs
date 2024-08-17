use async_lsp::lsp_types::SymbolKind;
use tree_sitter::Node;

#[allow(unused)]
pub enum NodeKind {
    Identifier,
    Error,
    MessageName,
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
            NodeKind::EnumName => "enum_name",
            NodeKind::FieldName => "message_or_enum_type",
            NodeKind::ServiceName => "service_name",
            NodeKind::RpcName => "rpc_name",
            NodeKind::PackageName => "package_name",
        }
    }

    pub fn is_identifier(n: &Node) -> bool {
        n.kind() == "identifier"
    }

    pub fn is_error(n: &Node) -> bool {
        n.kind() == "ERROR"
    }

    pub fn is_userdefined(n: &Node) -> bool {
        matches!(n.kind(), "message_name" | "enum_name")
    }

    pub fn is_actionable(n: &Node) -> bool {
        matches!(
            n.kind(),
            "message_name" | "enum_name" | "message_or_enum_type" | "rpc_name" | "service_name"
        )
    }

    pub fn to_symbolkind(n: &Node) -> SymbolKind {
        match n.kind() {
            "message_name" => SymbolKind::STRUCT,
            "enum_name" => SymbolKind::ENUM,
            _ => SymbolKind::NULL,
        }
    }
}
