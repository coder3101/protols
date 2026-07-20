`google.protobuf.Mixin`

---

**Well-Known Type**

Declares an API Interface to be included in this interface.

**Fields:**

- `name`: The fully qualified name of the API which is included.
- `root`: If non-empty specifies a path under which inherited HTTP paths are rooted.

```protobuf
message Mixin {
  string name = 1;
  string root = 2;
}
```
