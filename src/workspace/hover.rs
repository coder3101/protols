use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use async_lsp::lsp_types::{MarkupContent, MarkupKind};

use crate::{
    context::hoverable::Hoverables, state::ProtoLanguageState, utils::split_identifier_package,
};

static BUITIN_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        (
            "int32",
            r#"*int32* builtin type, A 32-bit integer (varint encoding)
---
Values of this type range between `-2147483648` and `2147483647`.
Beware that negative values are encoded as five bytes on the wire!"#,
        ),
        (
            "int64",
            r#"*int64* builtin type, A 64-bit integer (varint encoding)
---
Values of this type range between `-9223372036854775808` and `9223372036854775807`.
Beware that negative values are encoded as ten bytes on the wire!"#,
        ),
        (
            "uint32",
            r#"*uint32* builtin type, A 32-bit unsigned integer (varint encoding)
---
Values of this type range between `0` and `4294967295`."#,
        ),
        (
            "uint64",
            r#"*uint64* builtin type, A 64-bit unsigned integer (varint encoding)
---
Values of this type range between `0` and `18446744073709551615`."#,
        ),
        (
            "sint32",
            r#"*sint32* builtin type, A 32-bit integer (ZigZag encoding)
---
Values of this type range between `-2147483648` and `2147483647`."#,
        ),
        (
            "sint64",
            r#"*sint64* builtin type, A 64-bit integer (ZigZag encoding)
---
Values of this type range between `-9223372036854775808` and `9223372036854775807`."#,
        ),
        (
            "fixed32",
            r#"*fixed32* builtin type, A 32-bit unsigned integer (4-byte encoding)

Values of this type range between `0` and `4294967295`."#,
        ),
        (
            "fixed64",
            r#"*fixed64* builtin type, A 64-bit unsigned integer (8-byte encoding)
---
Values of this type range between `0` and `18446744073709551615`."#,
        ),
        (
            "sfixed32",
            r#"*sfixed64* builtin type, A 32-bit integer (4-byte encoding)
---
Values of this type range between `-2147483648` and `2147483647`."#,
        ),
        (
            "sfixed64",
            r#"*sfixed64* builtin type, A 64-bit integer (8-byte encoding)
---
Values of this type range between `-9223372036854775808` and `9223372036854775807`."#,
        ),
        (
            "float",
            r#"*float* builtin type
---
A single-precision floating point number (IEEE-745.2008 binary32)."#,
        ),
        (
            "double",
            r#"*double* builtin type,
---
A double-precision floating point number (IEEE-745.2008 binary64)."#,
        ),
        (
            "string",
            r#"*string* builtin type, A string of text.
---
Stores at most 4GB of text. Intended to be UTF-8 encoded Unicode; use `bytes` if you need other encodings."#,
        ),
        (
            "bytes",
            r#"*bytes* builtin type, A blob of arbitrary bytes.
---
Stores at most 4GB of binary data. Encoded as base64 in JSON."#,
        ),
        (
            "bool",
            r#"*bool* builtin type, A Boolean value: `true` or `false`.
---
Encoded as a single byte: `0x00` or `0xff` (all non-zero bytes decode to `true`)."#,
        ),
        (
            "default",
            r#"*default* builtin type, A magic option that specifies the field's default value.
---
Unlike every other option on a field, this does not have a corresponding field in
`google.protobuf.FieldOptions`; it is implemented by compiler magic."#,
        ),
    ])
});

static WELLKNOWN_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        (
            "google.protobuf.Any",
            r#"*google.protobuf.Any* wellknown type
---
`Any` contains an arbitrary serialized message along with a URL that describes the type of the serialized message.
The JSON representation of an Any value uses the regular representation of the deserialized, embedded message, with an additional field @type which contains the type URL.
---
```proto
message Any {
    string type_url = 1; // A URL/resource name that uniquely identifies the type of the serialized protocol buffer message
    bytes value = 2; // Must be a valid serialized protocol buffer
}
```

"#,
        ),
        (
            "google.protobuf.Api",
            r#"*google.protobuf.Api* well known type
---
`Api` is a light-weight descriptor for a protocol buffer service.
---
```proto
message Api {
    string name = 1; // The fully qualified name of this api, including package name followed by the api's simple name
    repeated Method methods = 2; // The methods of this api, in unspecified order
    repeated Option options = 3; // Any metadata attached to the API
    string version = 4; // A version string fo this interface
    SourceContext source_context = 5; // Source context for the protocol buffer service
    repeated Mixin mixins = 6; // Included interfaces
    Syntax syntax = 7; // Source syntax of the service
}
```

"#,
        ),
        (
            "google.protobuf.BoolValue",
            r#"*google.protobuf.BoolValue* well known type, Wrapper message for bool
---
The JSON representation for `BoolValue` is JSON `true` and `false`
---
```proto
message BoolValue {
    bool value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.BytesValue",
            r#"*google.protobuf.BytesValue* well known type, Wrapper message for bytes
---
The JSON representation for `BytesValue` is JSON string.
---
```proto
message BytesValue {
    bytes value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.DoubleValue",
            r#"*google.protobuf.DoubleValue* well known type, Wrapper message for double
---
The JSON representation for `DoubleValue` is JSON number.
---
```proto
message DoubleValue {
    double value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.Duration",
            r#"*google.protobuf.Duration* well known type
---
A Duration represents a signed, fixed-length span of time represented as a count of seconds and fractions of seconds at nanosecond resolution.
It is independent of any calendar and concepts like "day" or "month". 
It is related to Timestamp in that the difference between two Timestamp values is a Duration and it can be added or subtracted from a Timestamp. 
Range is approximately +-10,000 years.
---
```proto
message Duration {
    int64 seconds = 1; // Signed seconds of the span of time Must be from -315,576,000,000 to +315,576,000,000 inclusive
    int32 nanos = 2; // Signed fractions of a second at nanosecond resolution of the span of time. Durations less than one second are represented with a 0  `seconds` field and a positive or negative `nanos` field.
}
```
"#,
        ),
        (
            "google.protobuf.Empty",
            r#"*google.protobuf.Empty* well known type
---
A generic empty message that you can re-use to avoid defining duplicated empty messages in your APIs.
The JSON representation for Empty is empty JSON object `{}`
"#,
        ),
        (
            "google.protobuf.Enum",
            r#"*google.protobuf.Enum* well known type
---
Enum type definition
---
```proto
message Enum {
  string name = 1; // Enum type name.
  repeated EnumValue enumvalue = 2; // Enum value definitions.
  repeated Option options = 3; // Protocol buffer options.
  SourceContext source_context = 4; // The source context.
  Syntax syntax = 5; // The source syntax.
  string edition = 6; // The source edition string, only valid when syntax is SYNTAX_EDITIONS.
}
```
"#,
        ),
        (
            "google.protobuf.EnumValue",
            r#"*google.protobuf.EnumValue* well known type
---
Enum value definition
---
```proto
message EnumValue {
  string name = 1; // Enum value name.
  int32 number = 2; // Enum value number.
  repeated Option options = 3; // Protocol buffer options.
}

```
"#,
        ),
        (
            "google.protobuf.Field",
            r#"*google.protobuf.Field* well known type
---
A single field of a message type.
---
```proto
message Field {
  Kind kind = 1; // The field type.
  Cardinality cardinality = 2; // The field cardinality.
  int32 number = 3; // The field number.
  string name = 4; // The field name.
  string type_url = 6; // The field type URL, without the scheme, for message or enumeration types
  int32 oneof_index = 7; // The index of the field type in `Type.oneofs`, for message or enumeration types.
  bool packed = 8; // Whether to use alternative packed wire representation.
  repeated Option options = 9; // The protocol buffer options.
  string json_name = 10; // The field JSON name.
  string default_value = 11; // The string value of the default value of this field. Proto2 syntax only.
}
```
"#,
        ),
        (
            "google.protobuf.Field.Cardinality",
            r#"*google.protobuf.Field.Cardinality* well known type
---
Whether a field is optional, required, or repeated.
---
```proto
enum Cardinality {
    CARDINALITY_UNKNOWN = 0; // For fields with unknown cardinality.
    CARDINALITY_OPTIONAL = 1; // For optional fields.
    CARDINALITY_REQUIRED = 2; // For required fields. Proto2 syntax only.
    CARDINALITY_REPEATED = 3; // For repeated fields.
}

```
"#,
        ),
        (
            "google.protobuf.Field.Kind",
            r#"*google.protobuf.Field.Kind* well known type
---
Basic field types.
---
```proto
enum Kind {
    TYPE_UNKNOWN = 0; // Field type unknown.
    TYPE_DOUBLE = 1; // Field type double.
    TYPE_FLOAT = 2; // Field type float.
    TYPE_INT64 = 3; // Field type int64.
    TYPE_UINT64 = 4; // Field type uint64.
    TYPE_INT32 = 5; // Field type int32.
    TYPE_FIXED64 = 6; // Field type fixed64.
    TYPE_FIXED32 = 7; // Field type fixed32.
    TYPE_BOOL = 8; // Field type bool.
    TYPE_STRING = 9; // Field type string.
    TYPE_GROUP = 10; // Field type group. Proto2 syntax only, and deprecated.
    TYPE_MESSAGE = 11; // Field type message.
    TYPE_BYTES = 12; // Field type bytes.
    TYPE_UINT32 = 13; // Field type uint32.
    TYPE_ENUM = 14; // Field type enum.
    TYPE_SFIXED32 = 15; // Field type sfixed32.
    TYPE_SFIXED64 = 16; // Field type sfixed64.
    TYPE_SINT32 = 17; // Field type sint32.
    TYPE_SINT64 = 18; // Field type sint64.
}

```
"#,
        ),
        (
            "google.protobuf.FieldMask",
            r#"*google.protobuf.FieldMask* well known type
---
`FieldMask` represents a set of symbolic field paths
---
```proto
message FieldMask {
  repeated string paths = 1; // The set of field mask paths.
}

```
"#,
        ),
        (
            "google.protobuf.FloatValue",
            r#"*google.protobuf.FloatValue* well known type, Wrapper message for `float`
---
The JSON representation for `FloatValue` is JSON number.
---
```proto
message FloatValue {
    float value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.Int32Value",
            r#"*google.protobuf.Int32Value* well known type, Wrapper message for `int32`
---
The JSON representation for `Int32Value` is JSON number.
---
```proto
message Int32Value {
    int32 value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.Int64Value",
            r#"*google.protobuf.Int64Value* well known type, Wrapper message for `int64`
---
The JSON representation for `Int64Value` is JSON string.
---
```proto
message Int64Value {
    int64 value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.ListValue",
            r#"*google.protobuf.ListValue* well known type, Wrapper around a repeated field of values
---
The JSON representation for `ListValue` is JSON Array.
---
```proto
message Int64Value {
    Value values = 1;
}
```
"#,
        ),
        (
            "google.protobuf.Method",
            r#"*google.protobuf.Method* well known type
---
Method represents a method of an api.
---
```proto
message Method {
  string name = 1; // The simple name of this method.
  string request_type_url = 2; // A URL of the input message type.
  bool request_streaming = 3; // If true, the request is streamed.
  string response_type_url = 4; // The URL of the output message type.
  bool response_streaming = 5; // If true, the response is streamed.
  repeated Option options = 6; // Any metadata attached to the method.
  Syntax syntax = 7; // The source syntax of this method.
}
```
"#,
        ),
        (
            "google.protobuf.Mixin",
            r#"*google.protobuf.Mixin* well known type
---
Declares an API Interface to be included in this interface. The including
interface must redeclare all the methods from the included interface, but
documentation and options are inherited
---
```proto
message Mixin {
  string name = 1; // The fully qualified name of the interface which is included.
  string root = 2; // If non-empty specifies a path under which inherited HTTP paths are rooted.
}
```
"#,
        ),
        (
            "google.protobuf.NullValue",
            r#"*google.protobuf.NullValue* well known type
---
`NullValue` is a singleton enumeration to represent the null value for the Value type union.
The JSON representation for NullValue is JSON `null`.
---
```proto
enum NullValue {
    NULL_VALUE = 1;
}
```
"#,
        ),
        (
            "google.protobuf.Option",
            r#"*google.protobuf.Option* well known type
---
A protocol buffer option, which can be attached to a message, field, enumeration, etc
---
```proto
message Option {
    string name = 1; // The option's name
    Any value = 2; // The option's value
}
```
"#,
        ),
        (
            "google.protobuf.SourceContext",
            r#"*google.protobuf.SourceContext* well known type
---
`SourceContext` represents information about the source of a protobuf element, like the file in which it is defined
---
```proto
message SourceContext {
  string file_name = 1; // The path-qualified name of the .proto file that contained the associated protobuf element.
}
```
"#,
        ),
        (
            "google.protobuf.StringValue",
            r#"*google.protobuf.StringValue* well known type, Wrapper message for string.
---
The JSON representation for `StringValue` is JSON string.
---
```proto
message StringValue {
  string value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.Struct",
            r#"*google.protobuf.Struct* well known type
---
`Struct` represents a structured data value, consisting of fields
which map to dynamically typed values.
---
```proto
message Struct {
  map<string, Value> fields = 1; // Unordered map of dynamically typed values.
}
```
"#,
        ),
        (
            "google.protobuf.Syntax",
            r#"*google.protobuf.Syntax* well known type
---
The syntax in which a protocol buffer element is defined
---
```proto
enum Syntax {
    SYNTAX_PROTO2 = 1;
    SYNTAX_PROTO3 = 2;
    SYNTAX_EDITIONS = 3;
}
```
"#,
        ),
        (
            "google.protobuf.Timestamp",
            r#"*google.protobuf.Timestamp* well known type
---
`Timestamp` represents a point in time independent of any time zone or calendar, represented as seconds and fractions of seconds at nanosecond resolution in UTC Epoch time
---
```proto
message Timestamp {
  int64 seconds = 1; // Represents seconds of UTC time since Unix epoch
  int32 nanos = 2; // Non-negative fractions of a second at nanosecond resolution.
}
```
"#,
        ),
        (
            "google.protobuf.Type",
            r#"*google.protobuf.Type* well known type
---
A protocol buffer message type
---
```proto
message Type {
  string name = 1; // The fully qualified message name.
  repeated Field fields = 2; // The list of fields.
  repeated string oneofs = 3; // The list of types appearing in `oneof` definitions in this type.
  repeated Option options = 4; // The protocol buffer options.
  SourceContext source_context = 5; // The source context.
  Syntax syntax = 6; // The source syntax.
  string edition = 7; // The source edition string, only valid when syntax is SYNTAX_EDITIONS.
}
```
"#,
        ),
        (
            "google.protobuf.UInt32Value",
            r#"*google.protobuf.UInt32Value* well known type, Wrapper message for `uint32`
---
The JSON representation for `UInt32Value` is JSON number.
---
```proto
message UInt32Value {
    uint32 value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.UInt64Value",
            r#"*google.protobuf.UInt64Value* well known type, Wrapper message for `uint64`
---
The JSON representation for `UInt64Value` is JSON string.
---
```proto
message UInt64Value {
    uint64 value = 1;
}
```
"#,
        ),
        (
            "google.protobuf.Value",
            r#"*google.protobuf.Value* well known type
---
`Value` represents a dynamically typed value which can be either
null, a number, a string, a boolean, a recursive struct value, or a
list of values.

The JSON representation for `Value` is JSON value.
---
```proto
message Value {
  oneof kind {
    NullValue null_value = 1; // Represents a null value.
    double number_value = 2; // Represents a double value.
    string string_value = 3; // Represents a string value.
    bool bool_value = 4; // Represents a boolean value.
    Struct struct_value = 5; // Represents a structured value.
    ListValue list_value = 6; // Represents a repeated `Value`.
  }
}
```
"#,
        ),
    ])
});

impl ProtoLanguageState {
    pub fn hover(
        &self,
        ipath: &[PathBuf],
        curr_package: &str,
        hv: Hoverables,
    ) -> Option<MarkupContent> {
        let v = match hv {
            Hoverables::FieldType(field) => {
                // Type is a builtin
                match BUITIN_DOCS.get(field.as_str()) {
                    Some(docs) => docs.to_string(),
                    _ => String::new(),
                }
            }
            Hoverables::ImportPath(path) => {
                if let Some(p) = ipath.iter().map(|p| p.join(&path)).find(|p| p.exists()) {
                    format!(
                        r#"Import: `{path}` protobuf file,
---
Included from {}"#,
                        p.to_string_lossy(),
                    )
                } else {
                    String::new()
                }
            }
            Hoverables::Identifier(identifier) => {
                let (mut package, identifier) = split_identifier_package(identifier.as_str());
                if package.is_empty() {
                    package = curr_package;
                }

                // Node is user defined type or well known type
                // If user defined,
                let mut result = WELLKNOWN_DOCS
                    .get(format!("{package}.{identifier}").as_str())
                    .map(|&s| s.to_string())
                    .unwrap_or_default();

                // If no well known was found; try parsing from trees.
                if result.is_empty() {
                    for tree in self.get_trees_for_package(package) {
                        let res = tree.hover(identifier, self.get_content(&tree.uri));

                        if res.is_empty() {
                            continue;
                        }

                        result = format!(
                            r#"`{identifier}` message or enum type, package: `{package}`
---
{}"#,
                            res[0].clone()
                        );
                        break;
                    }
                }

                result
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
            "com.workspace",
            Hoverables::Identifier("com.super.secret.SomeSecret".to_string())
        ));
    }
}
