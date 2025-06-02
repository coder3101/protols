*google.protobuf.Mixin* well known type
---
Declares an API Interface to be included in this interface.
---
```proto
message Mixin {
    string name = 1; // The fully qualified name of the interface which is included.
    string root = 2; // If non-empty specifies a path under which the interface is served.
}
```