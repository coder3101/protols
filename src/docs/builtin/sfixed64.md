`sfixed64`

---

**Built-in Type**

A signed 64-bit integer using fixed-width 8-byte encoding.

**Details:**

- **Wire format**: 8-byte fixed-width value (`fixed64`).
- **Range**: `-9,223,372,036,854,775,808` to `9,223,372,036,854,775,807` inclusive.
- **Efficiency**: More efficient than `sint64` if values are frequently greater than `72,057,594,037,927,936` in absolute value.
- **Go type**: `int64`
- **C++ type**: `int64_t`
- **Rust type**: `i64`
