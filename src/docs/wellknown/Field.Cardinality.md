`google.protobuf.Field.Cardinality`

---

**Well-Known Type**

Whether a field is optional, required, or repeated.

**Values:**

- `CARDINALITY_UNKNOWN`: For fields with unknown cardinality.
- `CARDINALITY_OPTIONAL`: For optional fields.
- `CARDINALITY_REQUIRED`: For required fields. Valid for `proto2` syntax only.
- `CARDINALITY_REPEATED`: For repeated fields.

```protobuf
enum Cardinality {
  CARDINALITY_UNKNOWN = 0;
  CARDINALITY_OPTIONAL = 1;
  CARDINALITY_REQUIRED = 2;
  CARDINALITY_REPEATED = 3;
}
```
