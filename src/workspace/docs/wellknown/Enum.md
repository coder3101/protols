*google.protobuf.Enum* well known type
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