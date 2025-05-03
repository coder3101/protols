*google.protobuf.Method* well known type
---
Method represents a method of an API interface.
---
```proto
message Method {
    string name = 1; // The simple name of this method.
    string request_type_url = 2; // A URL of the input message type.
    bool request_streaming = 3; // If true, the request is streamed.
    string response_type_url = 4; // The URL of the output message type.
    bool response_streaming = 5; // If true, the response is streamed.
    repeated Option options = 6; // Any metadata attached to the method.
    Syntax syntax = 7; // The source syntax of this method.
}
```