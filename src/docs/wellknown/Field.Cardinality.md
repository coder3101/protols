*google.protobuf.Field.Cardinality* well known type
---
Whether a field is optional, required, or repeated.
---
```proto
enum Cardinality {
    CARDINALITY_UNKNOWN = 0; // For fields with unknown cardinality.
    CARDINALITY_OPTIONAL = 1; // For optional fields.
    CARDINALITY_REQUIRED = 2; // For required fields. Proto2 syntax only.
    CARDINALITY_REPEATED = 3; // For repeated fields.
}
```