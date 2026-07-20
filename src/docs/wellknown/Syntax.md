`google.protobuf.Syntax`

---

**Well-Known Type**

The syntax in which a protocol buffer element is defined.

**Values:**

- `SYNTAX_PROTO2`: Syntax `proto2`.
- `SYNTAX_PROTO3`: Syntax `proto3`.
- `SYNTAX_EDITIONS`: Syntax uses the `edition` construct.

```protobuf
enum Syntax {
  SYNTAX_PROTO2 = 0;
  SYNTAX_PROTO3 = 1;
  SYNTAX_EDITIONS = 2;
}
```
