`google.protobuf.Api`

---

**Well-Known Type**

`Api` is a light-weight descriptor for a protocol buffer service.

**Fields:**

- `name`: The fully qualified name of this API, including package name followed by the API's simple name.
- `methods`: The methods of this API, in unspecified order.
- `options`: Any metadata attached to the API.
- `version`: A version string for this API.
- `source_context`: Source context for the protocol buffer service represented by this message.
- `mixins`: Included APIs.
- `syntax`: The source syntax of the service.

```protobuf
message Api {
  string name = 1;
  repeated Method methods = 2;
  repeated Option options = 3;
  string version = 4;
  SourceContext source_context = 5;
  repeated Mixin mixins = 6;
  Syntax syntax = 7;
}
```
