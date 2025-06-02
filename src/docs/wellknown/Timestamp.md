*google.protobuf.Timestamp* well known type
---
A Timestamp represents a point in time independent of any time zone or calendar, represented as seconds and fractions of seconds at nanosecond resolution in UTC Epoch time.

It is encoded using the Proleptic Gregorian Calendar which extends the Gregorian calendar backwards to year one. It is encoded assuming all minutes are 60 seconds long, i.e. leap seconds are "smeared" so that no leap second table is needed for interpretation. Range is from 0001-01-01T00:00:00Z to 9999-12-31T23:59:59.999999999Z.
---
```proto
message Timestamp {
    int64 seconds = 1; // Represents seconds of UTC time since Unix epoch 1970-01-01T00:00:00Z
    int32 nanos = 2; // Non-negative fractions of a second at nanosecond resolution
}
```