*google.protobuf.Value* well known type
---
`Value` represents a dynamically typed value which can be either null, a number, a string, a boolean, a recursive struct value, or a list of values.
---
```proto
message Value {
    oneof kind {
        NullValue null_value = 1;
        double number_value = 2;
        string string_value = 3;
        bool bool_value = 4;
        Struct struct_value = 5;
        ListValue list_value = 6;
    }
}
```