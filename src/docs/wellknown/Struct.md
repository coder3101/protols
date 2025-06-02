*google.protobuf.Struct* well known type
---
`Struct` represents a structured data value, consisting of fields which map to dynamically typed values.
---
```proto
message Struct {
    map<string, Value> fields = 1; // Unordered map of dynamically typed values.
}
```