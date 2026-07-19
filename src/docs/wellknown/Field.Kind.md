`google.protobuf.Field.Kind`

---

**Well-Known Type**

Basic field types.

**Values:**

- `TYPE_UNKNOWN`: Field type unknown.
- `TYPE_GROUP`: Field type group. Valid for `proto2` syntax only, and deprecated.
- All other values (`TYPE_DOUBLE`, `TYPE_FLOAT`, etc.) represent their respective scalar or composite protocol buffer types.

```protobuf
enum Kind {
  TYPE_UNKNOWN = 0;
  TYPE_DOUBLE = 1;
  TYPE_FLOAT = 2;
  TYPE_INT64 = 3;
  TYPE_UINT64 = 4;
  TYPE_INT32 = 5;
  TYPE_FIXED64 = 6;
  TYPE_FIXED32 = 7;
  TYPE_BOOL = 8;
  TYPE_STRING = 9;
  TYPE_GROUP = 10;
  TYPE_MESSAGE = 11;
  TYPE_BYTES = 12;
  TYPE_UINT32 = 13;
  TYPE_ENUM = 14;
  TYPE_SFIXED32 = 15;
  TYPE_SFIXED64 = 16;
  TYPE_SINT32 = 17;
  TYPE_SINT64 = 18;
}
```
