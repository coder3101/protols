`google.protobuf.Method`

---

**Well-Known Type**

`Method` represents a method of an API interface.

**Fields:**

- `name`: The simple name of this method.
- `request_type_url`: A URL of the input message type.
- `request_streaming`: If `true`, the request is streamed.
- `response_type_url`: The URL of the output message type.
- `response_streaming`: If `true`, the response is streamed.
- `options`: Any metadata attached to the method.
- `syntax`: The source syntax of this method.

```protobuf
message Method {
  string name = 1;
  string request_type_url = 2;
  bool request_streaming = 3;
  string response_type_url = 4;
  bool response_streaming = 5;
  repeated Option options = 6;
  Syntax syntax = 7;
}
```
