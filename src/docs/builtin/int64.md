`int64`

---

**Built-in Type**

A signed 64-bit integer using variable-length encoding.

**Details:**

- **Wire format**: Variable-length value (`varint`).
- **Range**: `-9,223,372,036,854,775,808` to `9,223,372,036,854,775,807` inclusive.
- **Efficiency**: Inefficient for negative numbers. Negative values are sign-extended and take a full `10 bytes` on the wire. Use `sint64` instead if your field frequently contains negative numbers.
- **Go type**: `int64`
- **C++ type**: `int64_t`
- **Rust type**: `i64`
