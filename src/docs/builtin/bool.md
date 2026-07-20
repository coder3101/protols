`bool`

---

**Built-in Type**

A boolean value which can be either `true` or `false`.

**Details:**

- **Wire format**: Encoded as a `varint`.
- **Value mapping**: `0` decodes to `false`, and `1` decodes to `true` (any non-zero varint value decodes to `true`).
- **Go type**: `bool`
- **C++ type**: `bool`
- **Rust type**: `bool`
