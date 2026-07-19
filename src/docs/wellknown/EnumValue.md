`google.protobuf.EnumValue`

---

**Well-Known Type**

Enum value definition.

**Fields:**

- `name`: Enum value name.
- `number`: Enum value number.
- `options`: Protocol buffer options.

```protobuf
message EnumValue {
  string name = 1;
  int32 number = 2;
  repeated Option options = 3;
}
```
