`google.protobuf.ListValue`

---

**Well-Known Type**

`ListValue` is a wrapper around a repeated field of values.

The JSON representation for `ListValue` is JSON array.

**Fields:**

- `values`: Repeated field of dynamically typed values.

```protobuf
message ListValue {
  repeated Value values = 1;
}
```
