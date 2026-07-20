`google.protobuf.Enum`

---

**Well-Known Type**

Enum type definition.

**Fields:**

- `name`: Enum type name.
- `enumvalue`: Enum value definitions.
- `options`: Protocol buffer options.
- `source_context`: The source context.
- `syntax`: The source syntax.
- `edition`: The source edition if `syntax` is `SYNTAX_EDITIONS`.

```protobuf
message Enum {
  string name = 1;
  repeated EnumValue enumvalue = 2;
  repeated Option options = 3;
  SourceContext source_context = 4;
  Syntax syntax = 5;
  string edition = 6;
}
```
