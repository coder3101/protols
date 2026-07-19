`google.protobuf.Option`

---

**Well-Known Type**

A protocol buffer option, which can be attached to a message, field, enumeration, etc.

**Fields:**

- `name`: The option's name. For example, `"java_package"`.
- `value`: The option's value. For example, `"com.google.protobuf"`.

```protobuf
message Option {
  string name = 1;
  Any value = 2;
}
```
