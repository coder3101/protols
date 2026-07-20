`google.protobuf.SourceContext`

---

**Well-Known Type**

`SourceContext` represents information about the source of a protobuf element, like the file in which it is defined.

**Fields:**

- `file_name`: The path-qualified name of the `.proto` file that contained the associated protobuf element. For example: `"google/protobuf/source.proto"`.

```protobuf
message SourceContext {
  string file_name = 1;
}
```
