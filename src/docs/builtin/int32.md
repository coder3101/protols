`int32`

---

**Built-in Type**

A signed 32-bit integer using variable-length encoding.

**Details:**

- **Wire format**: Variable-length value (`varint`).
- **Range**: `-2,147,483,648` to `2,147,483,647` inclusive.
- **Efficiency**: Inefficient for negative numbers. Negative values are sign-extended and take a full `10 bytes` on the wire. Use `sint32` instead if your field frequently contains negative numbers.
- **Go type**: `int32`
- **C++ type**: `int32_t`
- **Rust type**: `i32`
