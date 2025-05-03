*google.protobuf.Any* wellknown type
---
`Any` contains an arbitrary serialized message along with a URL that describes the type of the serialized message.
The JSON representation of an Any value uses the regular representation of the deserialized, embedded message, with an additional field @type which contains the type URL.
---
```proto
message Any {
    string type_url = 1; // A URL/resource name that uniquely identifies the type of the serialized protocol buffer message
    bytes value = 2; // Must be a valid serialized protocol buffer
}
```