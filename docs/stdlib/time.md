# time: clocks, dates and sleeping

**Status in 2.0.0.** Implemented. The module is implemented in
`stdlib/src/time.ls`. Time points are `i64` values, never floating-point
seconds.

## Clock and sleep

| Function | Signature | Meaning |
| --- | --- | --- |
| `now_ms` | `() -> i64` | wall-clock time in milliseconds since the Unix epoch |
| `monotonic_ns` | `() -> i64` | monotonic clock in nanoseconds; only differences are meaningful |
| `sleep_ms` | `(i64)` | suspends the calling thread for at least the given milliseconds |

`now_ms` follows the system clock: it can jump forwards or backwards when the
clock is adjusted or when the machine wakes from sleep. `monotonic_ns` never
goes backwards and is the clock to use for durations and timeouts; it wraps after
about 292 years of uptime, which no practical program observes.

Sleep is blocking. lsaca has no async runtime, so `sleep_ms` stops the process
until the delay elapses. The sleep lasts at least the requested duration and may
be slightly longer because of scheduling.

## Dates and times

`DateTime` is a plain struct of `i64` fields. It is `Copy`, because a struct
whose fields are all `Copy` is `Copy` (see [Ownership and
moves](../language/11-ownership-and-moves.md)):

```rust
pub struct DateTime {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    weekday: i64,
    yearday: i64,
}
```

| Function | Signature | Meaning |
| --- | --- | --- |
| `DateTime::now_local` | `() -> Result<DateTime, io::Error>` | the current date and time in the local time zone |
| `DateTime::now_utc` | `() -> Result<DateTime, io::Error>` | the current date and time in UTC |

The conversion can fail when the operating system cannot provide a calendar
breakdown; the error carries the runtime classification.

| Accessor | Returns | Range |
| --- | --- | --- |
| `year` | `i64` | the full year (`2026`, not `126`) |
| `month` | `i64` | `1`-`12` |
| `day` | `i64` | `1`-`31` |
| `hour` | `i64` | `0`-`23` |
| `minute` / `second` | `i64` | `0`-`59` |
| `weekday` | `i64` | `0` Sunday through `6` Saturday |
| `yearday` | `i64` | zero-based day of the year |

Formatting helpers return owned `String` values:

| Method | Returns | Example |
| --- | --- | --- |
| `format_date` | `String` | `2026-11-02` |
| `format_time` | `String` | `13:45:09` |
| `format_datetime` | `String` | `2026-11-02 13:45:09` |
| `weekday_name` | `String` | `Monday` |

## Cross-platform notes

Local time comes from the operating system: time-zone rules and daylight-saving
changes are whatever the host reports, so the same instant can have a different
local `DateTime` on two machines. The conversion from milliseconds to calendar
fields is implemented in lsaca on top of the `__time_local_parts` and
`__time_utc_parts` intrinsics, so apart from the time zone the results are
identical on Windows, macOS and Linux.

## Limits in 2.0

There are no timer or async APIs, no `Duration` or `Instant` types, no
time-zone database and no date parsing in 2.0.

## See also

* [process](process.md) and [env](env.md) - the other operating-system modules.
* [Standard library chapter](../language/15-standard-library.md#io-and-process) -
  the 1.7 I/O and process free functions.
