`sint64`

---

**Built-in Type**

A signed 64-bit integer using variable-length ZigZag encoding.

**Details:**

- **Wire format**: Variable-length value (`varint`).
- **Encoding**: Uses ZigZag encoding to map negative numbers to positive numbers before varint compression.
- **Range**: `-9,223,372,036,854,775,808` to `9,223,372,036,854,775,807` inclusive.
- **Efficiency**: Highly efficient for negative numbers with small absolute values. Unlike `int64`, negative values do not take a full `10 bytes` on the wire.
- **Go type**: `int64`
- **C++ type**: `int64_t`
- **Rust type**: `i64`
