`google.protobuf.Any`

---

**Well-Known Type**

`Any` contains an arbitrary serialized message along with a URL that describes the type of the serialized message.

The JSON representation of an `Any` value uses the regular representation of the deserialized, embedded message, with an additional field `@type` which contains the type URL.

**Fields:**

- `type_url`: A URL or resource name that uniquely identifies the type of the serialized protocol buffer message.
- `value`: Must be a valid serialized protocol buffer.

```protobuf
message Any {
  string type_url = 1;
  bytes value = 2;
}
```
