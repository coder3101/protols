`google.protobuf.Field`

---

**Well-Known Type**

A single field of a message type.

**Fields:**

- `kind`: The field type.
- `cardinality`: The field cardinality.
- `number`: The field number.
- `name`: The field name.
- `type_url`: The field type URL, without the scheme, for message or enumeration types.
- `oneof_index`: The index of the field type in `Type.oneofs`, for message or enumeration types. The first type has index 1; zero means the type is not in the list.
- `packed`: Whether to use alternative packed wire representation.
- `options`: The protocol buffer options.
- `json_name`: The field JSON name.
- `default_value`: The string value of the default value of this field. Valid for `proto2` syntax only.

```protobuf
message Field {
  Kind kind = 1;
  Cardinality cardinality = 2;
  int32 number = 3;
  string name = 4;
  string type_url = 6;
  int32 oneof_index = 7;
  bool packed = 8;
  repeated Option options = 9;
  string json_name = 10;
  string default_value = 11;
}
```
