*google.protobuf.ListValue* well known type
---
`ListValue` is a wrapper around a repeated field of values.
The JSON representation for `ListValue` is JSON array.
---
```proto
message ListValue {
    repeated Value values = 1; // Repeated field of dynamically typed values.
}
```