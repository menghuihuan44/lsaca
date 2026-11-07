# time: clocks, sleeps and dates

**Status in 2.2.0.** Implemented in `stdlib/src/time.ls`. Since 2.1.0 the module
provides the Rust-shaped values `Duration`, `Instant` and `SystemTime` plus
`sleep(Duration)`. The older `now_ms`, `monotonic_ns`, `sleep_ms` and
`DateTime` helpers remain for compatibility and are marked deprecated in the
source; new code should use the new types.

Failures are [`io::Error`](io.md) values, never panics: the wall clock can move
backwards (NTP or a manual change), and the operations that compare wall clock
readings report `ErrorKind::InvalidInput` instead of producing a negative
duration.

## Duration

A span of time, split into whole seconds and a subsecond part in nanoseconds.
The fields are private; build durations with the constructors.

```rust
pub struct Duration {
    seconds: u64,
    nanos: u32,
}
```

| Method | Signature | Notes |
| --- | --- | --- |
| `Duration::new` | `(seconds: u64, nanos: u32) -> Duration` | nanoseconds at or above one billion roll over into whole seconds |
| `from_secs` | `(u64) -> Duration` | whole seconds |
| `from_millis` | `(u64) -> Duration` | whole milliseconds |
| `from_micros` | `(u64) -> Duration` | whole microseconds |
| `from_nanos` | `(u64) -> Duration` | whole nanoseconds |
| `as_secs` | `(&self) -> u64` | whole seconds |
| `subsec_nanos` | `(&self) -> u32` | the subsecond part |
| `as_millis` / `as_micros` / `as_nanos` | `(&self) -> u64` | the whole span in that unit, saturating at the largest `u64` |
| `is_zero` | `(&self) -> bool` | whether the duration is zero |
| `checked_add` | `(&self, other: Duration) -> Option<Duration>` | `None` on overflow |
| `checked_sub` | `(&self, other: Duration) -> Option<Duration>` | `None` when the result would be negative |

## Instant

A reading of the monotonic clock, for measuring elapsed time. `Instant` is a
`Copy` value, not a handle.

| Method | Signature | Notes |
| --- | --- | --- |
| `Instant::now` | `() -> Instant` | the current monotonic reading |
| `elapsed` | `(&self) -> Duration` | time since this reading; a clock that went backwards yields zero |
| `duration_since` | `(&self, earlier: Instant) -> Duration` | time between `earlier` and this reading, saturating at zero |
| `checked_duration_since` | `(&self, earlier: Instant) -> Option<Duration>` | `None` when `earlier` is later than this reading |

## SystemTime

A wall clock reading measured from the Unix epoch, in milliseconds. `SystemTime`
is a `Copy` value, not a handle.

| Method | Signature | Notes |
| --- | --- | --- |
| `SystemTime::now` | `() -> SystemTime` | the current wall clock time |
| `from_unix_ms` | `(i64) -> SystemTime` | a time at the given milliseconds after the epoch |
| `unix_ms` | `(&self) -> i64` | milliseconds since the epoch |
| `elapsed` | `(&self) -> Result<Duration, Error>` | time since this reading; `InvalidInput` when the clock moved backwards |
| `duration_since` | `(&self, earlier: SystemTime) -> Result<Duration, Error>` | time between the two readings; `InvalidInput` when `earlier` is later |
| `checked_duration_since` | `(&self, earlier: SystemTime) -> Option<Duration>` | `None` when the clock moved backwards |
| `unix_epoch` | `() -> SystemTime` | the Unix epoch (`0` milliseconds) |

Rust spells the epoch as the constant `UNIX_EPOCH`; a lsaca `const` cannot have
a struct type in 2.2.0, so the standard library provides it as a function:

```rust
let start: SystemTime = unix_epoch();
```

## sleep

`sleep(duration: Duration)` suspends the current thread. The runtime sleeps in
whole milliseconds, so the duration is rounded down; a duration longer than the
largest `i64` millisecond count is clamped rather than overflowing. Sleeping for
zero returns immediately. There is no async timer type.

## Compatibility helpers

These remain available but are deprecated in 2.1.0:

| Helper | Signature | Replacement |
| --- | --- | --- |
| `now_ms` | `() -> i64` | `SystemTime::now().unix_ms()` |
| `monotonic_ns` | `() -> i64` | `Instant::now()` with `elapsed()` |
| `sleep_ms` | `(milliseconds: i64)` | `sleep(Duration::from_millis(...))` |
| `DateTime` | `now_local` / `now_utc` -> `Result<DateTime, Error>` | keep `SystemTime`, convert only for display |

`DateTime` breaks a wall clock value into parts: `year`, `month` (`1`-`12`),
`day`, `hour`, `minute`, `second`, `weekday` (`0` Sunday through `6` Saturday)
and `yearday` (`0` based day of the year). `format_date`, `format_time`,
`format_datetime` and `weekday_name` return fixed-format `String` text.

## Cross-platform notes

* The wall clock follows the operating system's clock and time zone; local time
  is the platform's local time.
* The monotonic clock is the platform's monotonic counter. Its origin is
  unspecified, so only differences between readings are meaningful.
* Millisecond resolution is the guaranteed granularity of the wall clock; the
  monotonic clock may be finer.
* Times before the Unix epoch have negative millisecond counts, and `modified()`
  in [fs](fs.md) reports an error for them.

## Limits in 2.2.0

There is no calendar arithmetic (add days or months to a date), no date parsing,
no formatting beyond the `DateTime` helpers, no time zone database beyond the
platform's local time, no `UNIX_EPOCH` constant and no negative `Duration`.
`DateTime` remains a compatibility type: new code should store `SystemTime` and
format only at the point of display.

## See also

* [io](io.md) - the `Error` type shared with the clock operations.
* [fs](fs.md) - `SystemTime` in file metadata.
* [Standard library chapter](../language/15-standard-library.md#io-and-process).
