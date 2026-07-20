use super::super::captures::{
    definitions::{
        ENUM, ENUM_FIELD, FIELD, IMPORT, MAP_FIELD, MESSAGE, ONEOF, ONEOF_FIELD, PACKAGE, RPC,
        SERVICE,
    },
    properties::{
        DEPRECATION_MARKER, DOC_COMMENT, ENUM_VALUE, IMPORT_PATH, NAME, OPTION_NAME, OPTION_VALUE,
        RPC_REQUEST_STREAM, RPC_RESPONSE_STREAM, TAG,
    },
    references::{FIELD_TYPE, MAP_KEY, MAP_VALUE, RPC_REQUEST, RPC_RESPONSE},
};

/// Generates the master Tree-sitter SCM (Source Code Matcher) query string used
/// for metadata and layout extraction across all protobuf structural elements.
///
/// This function constructs a unified, high-performance tree query that
/// patterns and captures messages, enums, services, fields, and independent
/// modifiers (like deprecation markers or docstrings).
///
/// # Returns
///
/// A monolithic [`String`] containing the raw Tree-sitter query DSL complete
/// with semantic capture handles (`@NAME`, `@{FIELD_TYPE}`,
/// `@{DEPRECATION_MARKER}`, etc.).
pub fn generate_metamodel_query() -> String {
    format!(
        r#"
(comment) @{DOC_COMMENT}

(package
    (full_ident) @{NAME}
) @{PACKAGE}

(import
    path: (string) @{IMPORT_PATH}
) @{IMPORT}

(service
    (service_name (identifier) @{NAME})
) @{SERVICE}

(rpc
    (rpc_name (identifier) @{NAME})
    ("stream")? @{RPC_REQUEST_STREAM}
    (message_or_enum_type) @{RPC_REQUEST}
    ("stream")? @{RPC_RESPONSE_STREAM}
    (message_or_enum_type) @{RPC_RESPONSE}
) @{RPC}

(message
    (message_name (identifier) @{NAME})
) @{MESSAGE}

(field
    (type) @{FIELD_TYPE}
    (identifier) @{NAME}
    (field_number) @{TAG}
) @{FIELD}

(map_field
    (key_type) @{MAP_KEY}
    (type) @{MAP_VALUE}
    (identifier) @{NAME}
    (field_number) @{TAG}
) @{MAP_FIELD}

(oneof
    (identifier) @{NAME}
) @{ONEOF}

(oneof_field
    (type) @{FIELD_TYPE}
    (identifier) @{NAME}
    (field_number) @{TAG}
) @{ONEOF_FIELD}

(enum
    (enum_name (identifier) @{NAME})
) @{ENUM}

(enum_field
    (identifier) @{NAME}
    (int_lit) @{ENUM_VALUE}
) @{ENUM_FIELD}

(option
    (identifier) @{OPTION_NAME}
    (constant) @{OPTION_VALUE}
    (#eq? @{OPTION_NAME} "deprecated")
    (#eq? @{OPTION_VALUE} "true")
) @{DEPRECATION_MARKER}

 (field_option
    (identifier) @{OPTION_NAME}
    (constant) @{OPTION_VALUE}
    (#eq? @{OPTION_NAME} "deprecated")
    (#eq? @{OPTION_VALUE} "true")
) @{DEPRECATION_MARKER}

(enum_value_option
    (identifier) @{OPTION_NAME}
    (constant) @{OPTION_VALUE}
    (#eq? @{OPTION_NAME} "deprecated")
    (#eq? @{OPTION_VALUE} "true")
) @{DEPRECATION_MARKER}
"#
    )
}
