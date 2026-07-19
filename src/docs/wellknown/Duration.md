`google.protobuf.Duration`

---

**Well-Known Type**

A `Duration` represents a signed, fixed-length span of time represented as a count of seconds and fractions of seconds at nanosecond resolution.

It is independent of any calendar and concepts like "day" or "month". It is related to `Timestamp` in that the difference between two `Timestamp` values is a `Duration` and it can be added or subtracted from a `Timestamp`. Range is approximately ±10,000 years.

**Fields:**

- `seconds`: Signed seconds of the span of time. Must be from `-315,576,000,000` to `+315,576,000,000` inclusive.
- `nanos`: Signed fractions of a second at nanosecond resolution of the span of time. Durations less than one second are represented with a 0 `seconds` field and a positive or negative `nanos` field. For durations of one second or more, a non-zero value for the `nanos` field must be of the same sign as the `seconds` field. Must be from `-999,999,999` to `+999,999,999` inclusive.

```protobuf
message Duration {
  int64 seconds = 1;
  int32 nanos = 2;
}
```
