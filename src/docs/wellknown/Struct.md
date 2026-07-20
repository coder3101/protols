`google.protobuf.Struct`

---

**Well-Known Type**

`Struct` represents a structured data value, consisting of fields which map to dynamically typed values.

The JSON representation for `Struct` is a JSON object.

**Fields:**

- `fields`: Map of dynamically typed values.

```protobuf
message Struct {
  map<string, Value> fields = 1;
}
```
