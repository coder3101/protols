`string`

---

**Built-in Type**

A string of text.

**Details:**

- **Wire format**: Length-delimited.
- **Encoding**: Must always contain UTF-8 encoded or 7-bit ASCII text. Use `bytes` if you need to store arbitrary binary data or other encodings.
- **Capacity**: Stores at most 4 GiB of text.
- **Go type**: `string`
- **C++ type**: `std::string`
- **Rust type**: `::std::string::String`
