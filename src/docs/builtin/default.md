`default`

---

**Built-in Option**

A pseudo-option that specifies a custom default value for a non-repeated scalar field when the field is absent.

Unlike every other option on a field, this does not have a corresponding field in `google.protobuf.FieldOptions`. It is handled directly by compiler internals.

**Details:**

- **Syntax context**: Exclusively used in `proto2` syntax (or as a feature override in newer Protocol Buffers Editions).
- **Behavior**: Without a custom `default`, absent fields automatically fallback to the implicit zero-value of their respective scalar type (e.g., `0` for integers, `""` for strings).
