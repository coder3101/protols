*google.protobuf.EnumValue* well known type
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