`sint32`

---

**Built-in Type**

A signed 32-bit integer using variable-length ZigZag encoding.

**Details:**

- **Wire format**: Variable-length value (`varint`).
- **Encoding**: Uses ZigZag encoding to map negative numbers to positive numbers before varint compression.
- **Range**: `-2,147,483,648` to `2,147,483,647` inclusive.
- **Efficiency**: Highly efficient for negative numbers with small absolute values. Unlike `int32`, negative values do not take a full `10 bytes` on the wire.
- **Go type**: `int32`
- **C++ type**: `int32_t`
- **Rust type**: `i32`
