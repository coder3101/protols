*google.protobuf.Api* well known type
---
`Api` is a light-weight descriptor for a protocol buffer service.
---
```proto
message Api {
    string name = 1; // The fully qualified name of this api, including package name followed by the api's simple name
    repeated Method methods = 2; // The methods of this api, in unspecified order
    repeated Option options = 3; // Any metadata attached to the API
    string version = 4; // A version string fo this interface
    SourceContext source_context = 5; // Source context for the protocol buffer service
    repeated Mixin mixins = 6; // Included interfaces
    Syntax syntax = 7; // Source syntax of the service
}
```