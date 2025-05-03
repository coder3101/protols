*google.protobuf.Type* well known type
---
A protocol buffer message type.
---
```proto
message Type {
    string name = 1; // The fully qualified message name
    repeated Field fields = 2; // The list of fields
    repeated string oneofs = 3; // The list of types appearing in `oneof` definitions in this type
    repeated Option options = 4; // The protocol buffer options
    SourceContext source_context = 5; // The source context
    Syntax syntax = 6; // The source syntax
    string edition = 7; // The source edition string, only valid when syntax is SYNTAX_EDITIONS
}
```