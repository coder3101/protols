`fixed64`

---

**Built-in Type**

A 64-bit unsigned integer using fixed-width 8-byte encoding.

**Details:**

- **Wire format**: 8-byte fixed-width value (`fixed64`).
- **Range**: `0` to `18,446,744,073,709,551,615` inclusive.
- **Efficiency**: More efficient than `uint64` if values are frequently greater than `72,057,594,037,927,936`.
- **Go type**: `uint64`
- **C++ type**: `uint64_t`
- **Rust type**: `u64`
