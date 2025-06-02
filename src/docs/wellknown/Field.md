*google.protobuf.Field* well known type
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