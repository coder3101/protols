// These hover documentation comes from: https://protobuf.dev/reference/protobuf/google.protobuf/#index
pub fn hover_on(t: &str) -> Option<&'static str> {
    if let Some(field) = t.strip_prefix("google.protobuf.") {
        return match field {
            "Any" => Some("Any contains an arbitrary serialized message along with a URL that describes the type of the serialized message."),
            "Api" => Some("Api is a light-weight descriptor for a protocol buffer service."),
            "BoolValue" => Some("Wrapper message for bool"),
            "BytesValue" => Some("Wrapper message for bytes"),
            "DoubleValue" => Some("Wrapper message for double"),
            "Duration" => Some(r#"A Duration represents a signed, fixed-length span of time represented as a count of seconds and fractions of seconds at nanosecond resolution. It is independent of any calendar and concepts like "day" or "month". It is related to Timestamp in that the difference between two Timestamp values is a Duration and it can be added or subtracted from a Timestamp. Range is approximately +-10,000 years"#),
            "Empty" => Some("A generic empty message that you can re-use to avoid defining duplicated empty messages in your APIs. A typical example is to use it as the request or the response type of an API method"),
            "Enum" => Some("Enum type definition"),
            "EnumValue" => Some("Enum value definition"),
            "Field" => Some("A single field of a message type"),
            "Cardinality" => Some("Whether a field is optional, required, or repeated"),
            "Kind" => Some("Basic field types"),
            "FieldMask" => Some("FieldMask represents a set of symbolic field paths"),
            "FloatValue" => Some("Wrapper message for float"),
            "Int32Value" => Some("Wrapper message for int32"),
            "Int64Value" => Some("Wrapper message for int64"),
            "ListValue" => Some("ListValue is a wrapper around a repeated field of values"),
            "Method" => Some("Method represents a method of an api"),
            "Mixin" => Some("Declares an API to be included in this API"),
            "NullValue" => Some("NullValue is a singleton enumeration to represent the null value for the Value type union"),
            "Option" => Some("A protocol buffer option, which can be attached to a message, field, enumeration, etc"),
            "SourceContext" => Some("SourceContext represents information about the source of a protobuf element, like the file in which it is defined"),
            "StringValue" => Some("Wrapper message for string"),
            "Struct" => Some("Struct represents a structured data value, consisting of fields which map to dynamically typed values"),
            "Syntax" => Some("The syntax in which a protocol buffer element is defined"),
            "Timestamp" => Some(r#"A Timestamp represents a point in time independent of any time zone or calendar, represented as seconds and fractions of seconds at nanosecond resolution in UTC Epoch time. It is encoded using the Proleptic Gregorian Calendar which extends the Gregorian calendar backwards to year one. It is encoded assuming all minutes are 60 seconds long, i.e. leap seconds are "smeared" so that no leap second table is needed for interpretation. Range is from 0001-01-01T00:00:00Z to 9999-12-31T23:59:59.999999999Z. By restricting to that range, we ensure that we can convert to and from RFC 3339 date strings"#),
            "Type" => Some("A protocol buffer message type"),
            "UInt32Value" => Some("Wrapper message for uint32"),
            "UInt64Value" => Some("Wrapper message for uint64"),
            "Value" => Some("Value represents a dynamically typed value which can be either null, a number, a string, a boolean, a recursive struct value, or a list of values. A producer of value is expected to set one of that variants, absence of any variant indicates an error"),
        _ => None,
    };
    }
    None
}
