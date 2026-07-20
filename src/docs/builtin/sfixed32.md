`sfixed32`

---

**Built-in Type**

A signed 32-bit integer using fixed-width 4-byte encoding.

**Details:**

- **Wire format**: 4-byte fixed-width value (`fixed32`).
- **Range**: `-2,147,483,648` to `2,147,483,647` inclusive.
- **Efficiency**: More efficient than `sint32` if values are frequently greater than `268,435,456` in absolute value.
- **Go type**: `int32`
- **C++ type**: `int32_t`
- **Rust type**: `i32`
