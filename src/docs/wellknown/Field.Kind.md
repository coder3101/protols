*google.protobuf.Field.Kind* well known type
---
Basic field types.
---
```proto
enum Kind {
    TYPE_UNKNOWN = 0; // Field type unknown.
    TYPE_DOUBLE = 1; // Field type double.
    TYPE_FLOAT = 2; // Field type float.
    TYPE_INT64 = 3; // Field type int64.
    TYPE_UINT64 = 4; // Field type uint64.
    TYPE_INT32 = 5; // Field type int32.
    TYPE_FIXED64 = 6; // Field type fixed64.
    TYPE_FIXED32 = 7; // Field type fixed32.
    TYPE_BOOL = 8; // Field type bool.
    TYPE_STRING = 9; // Field type string.
    TYPE_GROUP = 10; // Field type group. Proto2 syntax only, and deprecated.
    TYPE_MESSAGE = 11; // Field type message.
    TYPE_BYTES = 12; // Field type bytes.
    TYPE_UINT32 = 13; // Field type uint32.
    TYPE_ENUM = 14; // Field type enum.
    TYPE_SFIXED32 = 15; // Field type sfixed32.
    TYPE_SFIXED64 = 16; // Field type sfixed64.
    TYPE_SINT32 = 17; // Field type sint32.
    TYPE_SINT64 = 18; // Field type sint64.
}
```