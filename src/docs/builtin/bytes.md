`bytes`

---

**Built-in Type**

A blob of arbitrary bytes.

**Details:**

- **Wire format**: Length-delimited.
- **Capacity**: Stores at most 4 GiB of binary data.
- **JSON format**: Encoded as a base64 string.
- **Go type**: `[]byte`
- **C++ type**: `std::string`
- **Rust type**: `::std::vec::Vec<u8>`
