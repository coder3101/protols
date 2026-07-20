`google.protobuf.Type`

---

**Well-Known Type**

A protocol buffer message type.

**Fields:**

- `name`: The fully qualified message name.
- `fields`: The list of fields.
- `oneofs`: The list of types appearing in `oneof` definitions in this type.
- `options`: The protocol buffer options.
- `source_context`: The source context.
- `syntax`: The source syntax.
- `edition`: The source syntax.

```protobuf
message Type {
  string name = 1;
  repeated Field fields = 2;
  repeated string oneofs = 3;
  repeated Option options = 4;
  SourceContext source_context = 5;
  Syntax syntax = 6;
  string edition = 7;
}
```
