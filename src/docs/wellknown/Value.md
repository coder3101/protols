`google.protobuf.Value`

---

**Well-Known Type**

`Value` represents a dynamically typed value which can be either `null`, a number, a string, a boolean, a recursive struct value, or a list of values.

The JSON representation for `Value` is JSON value.

**Fields:**

- `null_value`: Represents a `null` value.
- `number_value`: Represents a double value. Note that attempting to serialize `NaN` or `Infinity` results in error. (We can't serialize these as string `"NaN"` or `"Infinity"` values like we do for regular fields, because they would parse as `string_value`, not `number_value`).
- `string_value`: Represents a string value.
- `bool_value`: Represents a boolean value.
- `struct_value`: Represents a structured value.
- `list_value`: Represents a repeated `Value`.

```protobuf
message Value {
  oneof kind {
    NullValue null_value = 1;
    double number_value = 2;
    string string_value = 3;
    bool bool_value = 4;
    Struct struct_value = 5;
    ListValue list_value = 6;
  }
}
```
