*google.protobuf.SourceContext* well known type
---
`SourceContext` represents information about the source of a protobuf element, like the file in which it is defined.
---
```proto
message SourceContext {
    string file_name = 1; // The path-qualified name of the .proto file that contained the associated protobuf element.
}
```