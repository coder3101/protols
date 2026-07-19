`fixed32`

---

**Built-in Type**

A 32-bit unsigned integer using fixed-width 4-byte encoding.

**Details:**

- **Wire format**: 4-byte fixed-width value (`fixed32`).
- **Range**: `0` to `4,294,967,295` inclusive.
- **Efficiency**: More efficient than `uint32` if values are frequently greater than `268,435,456`.
- **Go type**: `uint32`
- **C++ type**: `uint32_t`
- **Rust type**: `u32`
