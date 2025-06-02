*google.protobuf.Option* well known type
---
A protocol buffer option, which can be attached to a message, field, enumeration, etc.
---
```proto
message Option {
    string name = 1; // The option's name. For example, "java_package".
    Any value = 2; // The option's value. For example, "com.google.protobuf".
}
```