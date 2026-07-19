/// Global string constants representing root structural elements in the
/// Tree-sitter query.
///
/// These handles identify the base container or terminal entities being parsed
/// (e.g., messages, fields, RPCs) and are used by the central dispatcher to
/// route query matches to their designated handlers.
pub mod definitions {
    pub const PACKAGE: &str = "element.package";
    pub const IMPORT: &str = "element.import";
    pub const MESSAGE: &str = "element.message";
    pub const FIELD: &str = "element.field";
    pub const ONEOF: &str = "element.oneof";
    pub const MAP_FIELD: &str = "element.map_field";
    pub const ONEOF_FIELD: &str = "element.oneof_field";
    pub const SERVICE: &str = "element.service";
    pub const RPC: &str = "element.rpc";
    pub const ENUM: &str = "element.enum";
    pub const ENUM_FIELD: &str = "element.enum_field";

    const ALL: &[&str] = &[
        PACKAGE,
        IMPORT,
        MESSAGE,
        FIELD,
        ONEOF,
        MAP_FIELD,
        ONEOF_FIELD,
        SERVICE,
        RPC,
        ENUM,
        ENUM_FIELD,
    ];

    /// Checks whether the provided capture handle represents a valid root
    /// element definition.
    #[inline]
    pub fn is_match(capture_name: &str) -> bool {
        ALL.contains(&capture_name)
    }
}

/// String constants representing data type signatures and external type
/// references.
///
/// These handles capture the positions and names of types referenced within
/// fields, maps, or RPC endpoints.
pub mod references {
    pub const FIELD_TYPE: &str = "field.type";
    pub const MAP_KEY: &str = "map_field.key";
    pub const MAP_VALUE: &str = "map_field.value";
    pub const RPC_REQUEST: &str = "rpc.request";
    pub const RPC_RESPONSE: &str = "rpc.response";
}

/// String constants representing metadata properties, options, and literals.
///
/// These handles capture auxiliary information attached to elements, such as
/// field tags, numeric enum constants, documentation blocks, or inline options.
pub mod properties {
    pub const NAME: &str = "name";
    pub const TAG: &str = "tag";
    pub const ENUM_VALUE: &str = "enum_field.value";
    pub const DOC_COMMENT: &str = "doc_comment";
    pub const DEPRECATION_MARKER: &str = "deprecation_marker";
    pub const IMPORT_PATH: &str = "import.path";
    pub const RPC_REQUEST_STREAM: &str = "rpc.request.stream";
    pub const RPC_RESPONSE_STREAM: &str = "rpc.response.stream";
    pub const OPTION_NAME: &str = "option.name";
    pub const OPTION_VALUE: &str = "option.value";
}
