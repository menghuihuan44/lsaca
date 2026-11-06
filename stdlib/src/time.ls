// Time helpers.
//
// Wall clock values are Unix milliseconds; the monotonic clock is a
// nanosecond counter suitable for measuring durations. Calendar parts are
// available in local time and in UTC. Failures are reported as io::Error
// values, never as panics.
//
// Since 2.1.0 the module provides the Rust-shaped Duration, Instant and
// SystemTime values plus sleep(Duration). The UNIX_EPOCH constant of Rust
// cannot be expressed because a const cannot have a struct type, so it is
// provided as the unix_epoch() function instead. The older now_ms,
// monotonic_ns, sleep_ms and DateTime helpers stay for compatibility.

pub extern fn __time_now_ms() -> i64;
pub extern fn __time_monotonic_ns() -> i64;
pub extern fn __time_sleep_ms(milliseconds: i64);
pub extern fn __time_local_parts(unix_ms: i64) -> (i64, i64, i64, i64, i64, i64, i64, i64, i64);
pub extern fn __time_utc_parts(unix_ms: i64) -> (i64, i64, i64, i64, i64, i64, i64, i64, i64);

/// The largest value of u64.
const U64_MAX: u64 = 18446744073709551615u64;

// ===========================================================================
// Duration
// ===========================================================================

/// A span of time, split into whole seconds and a subsecond part in
/// nanoseconds. The nanoseconds part is always below one billion.
pub struct Duration {
    seconds: u64,
    nanos: u32,
}

impl Duration {
    /// Creates a duration from whole seconds plus nanoseconds.
    ///
    /// Nanoseconds at or above one billion roll over into whole seconds
    /// instead of panicking.
    pub fn new(seconds: u64, nanos: u32) -> Duration {
        let extra: u64 = (nanos / 1000000000u32) as u64;
        let remaining: u32 = nanos % 1000000000u32;
        return Duration { seconds: seconds + extra, nanos: remaining };
    }

    /// Creates a duration of the given number of whole seconds.
    pub fn from_secs(seconds: u64) -> Duration {
        return Duration { seconds: seconds, nanos: 0 };
    }

    /// Creates a duration of the given number of milliseconds.
    pub fn from_millis(milliseconds: u64) -> Duration {
        let seconds: u64 = milliseconds / 1000u64;
        let remaining: u64 = milliseconds % 1000u64;
        return Duration { seconds: seconds, nanos: (remaining * 1000000u64) as u32 };
    }

    /// Creates a duration of the given number of microseconds.
    pub fn from_micros(microseconds: u64) -> Duration {
        let seconds: u64 = microseconds / 1000000u64;
        let remaining: u64 = microseconds % 1000000u64;
        return Duration { seconds: seconds, nanos: (remaining * 1000u64) as u32 };
    }

    /// Creates a duration of the given number of nanoseconds.
    pub fn from_nanos(nanoseconds: u64) -> Duration {
        let seconds: u64 = nanoseconds / 1000000000u64;
        let remaining: u64 = nanoseconds % 1000000000u64;
        return Duration { seconds: seconds, nanos: remaining as u32 };
    }

    /// The whole seconds part of the duration.
    pub fn as_secs(&self) -> u64 {
        return self.seconds;
    }

    /// The subsecond part in nanoseconds (always below one billion).
    pub fn subsec_nanos(&self) -> u32 {
        return self.nanos;
    }

    /// The duration in whole milliseconds, saturating at the largest u64.
    pub fn as_millis(&self) -> u64 {
        let fraction: u64 = (self.nanos / 1000000u32) as u64;
        if self.seconds > (U64_MAX - fraction) / 1000u64 {
            return U64_MAX;
        }
        return self.seconds * 1000u64 + fraction;
    }

    /// The duration in whole microseconds, saturating at the largest u64.
    pub fn as_micros(&self) -> u64 {
        let fraction: u64 = (self.nanos / 1000u32) as u64;
        if self.seconds > (U64_MAX - fraction) / 1000000u64 {
            return U64_MAX;
        }
        return self.seconds * 1000000u64 + fraction;
    }

    /// The duration in whole nanoseconds, saturating at the largest u64.
    pub fn as_nanos(&self) -> u64 {
        let fraction: u64 = self.nanos as u64;
        if self.seconds > (U64_MAX - fraction) / 1000000000u64 {
            return U64_MAX;
        }
        return self.seconds * 1000000000u64 + fraction;
    }

    /// Whether the duration is zero.
    pub fn is_zero(&self) -> bool {
        return self.seconds == 0 && self.nanos == 0;
    }

    /// Adds two durations, or returns None when the result does not fit u64.
    pub fn checked_add(&self, other: Duration) -> Option<Duration> {
        if other.seconds > U64_MAX - self.seconds {
            return None;
        }
        let mut seconds: u64 = self.seconds + other.seconds;
        let mut nanos: u32 = self.nanos;
        if other.nanos >= 1000000000u32 - nanos {
            if seconds == U64_MAX {
                return None;
            }
            seconds = seconds + 1;
            nanos = nanos + other.nanos - 1000000000u32;
        } else {
            nanos = nanos + other.nanos;
        }
        return Some(Duration { seconds: seconds, nanos: nanos });
    }

    /// Subtracts other from this duration, or returns None when other is
    /// larger than this duration.
    pub fn checked_sub(&self, other: Duration) -> Option<Duration> {
        if other.seconds > self.seconds {
            return None;
        }
        let mut seconds: u64 = self.seconds - other.seconds;
        let mut nanos: u32 = self.nanos;
        if other.nanos > nanos {
            if seconds == 0 {
                return None;
            }
            seconds = seconds - 1;
            nanos = nanos + 1000000000u32 - other.nanos;
        } else {
            nanos = nanos - other.nanos;
        }
        return Some(Duration { seconds: seconds, nanos: nanos });
    }
}

// ===========================================================================
// Instant
// ===========================================================================

/// A monotonic clock reading for measuring elapsed time. Instant values are
/// Copy: they are not handles and never need to be returned.
pub struct Instant {
    nanos: i64,
}

impl Instant {
    /// The current reading of the monotonic clock.
    pub fn now() -> Instant {
        return Instant { nanos: __time_monotonic_ns() };
    }

    /// The time elapsed since this reading.
    ///
    /// A clock that went backwards (which the monotonic clock should not do)
    /// yields a zero duration instead of a negative one.
    pub fn elapsed(&self) -> Duration {
        let now: i64 = __time_monotonic_ns();
        if now <= self.nanos {
            return Duration::from_secs(0);
        }
        return Duration::from_nanos((now - self.nanos) as u64);
    }

    /// The time elapsed between earlier and this reading, saturating at zero.
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        if self.nanos <= earlier.nanos {
            return Duration::from_secs(0);
        }
        return Duration::from_nanos((self.nanos - earlier.nanos) as u64);
    }

    /// The time elapsed between earlier and this reading, or None when
    /// earlier is later than this reading.
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        if self.nanos < earlier.nanos {
            return None;
        }
        return Some(Duration::from_nanos((self.nanos - earlier.nanos) as u64));
    }
}

// ===========================================================================
// SystemTime
// ===========================================================================

/// A wall clock reading measured from the Unix epoch. SystemTime values are
/// Copy: they are plain millisecond counts, not handles.
pub struct SystemTime {
    ms: i64,
}

impl SystemTime {
    /// The current wall clock time.
    pub fn now() -> SystemTime {
        return SystemTime { ms: __time_now_ms() };
    }

    /// A wall clock time at the given number of milliseconds after the Unix
    /// epoch (a negative value is before the epoch).
    pub fn from_unix_ms(milliseconds: i64) -> SystemTime {
        return SystemTime { ms: milliseconds };
    }

    /// The milliseconds since the Unix epoch (negative before the epoch).
    pub fn unix_ms(&self) -> i64 {
        return self.ms;
    }

    /// The time elapsed since this reading.
    ///
    /// A wall clock that went backwards (for example after a time adjustment)
    /// is reported as an InvalidInput error instead of a panic.
    pub fn elapsed(&self) -> Result<Duration, Error> {
        let now: i64 = __time_now_ms();
        if now < self.ms {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                String::from("SystemTime::elapsed: the wall clock went backwards"),
            ));
        }
        return Ok(Duration::from_millis((now - self.ms) as u64));
    }

    /// The time elapsed between earlier and this reading.
    pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, Error> {
        if self.ms < earlier.ms {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                String::from("SystemTime::duration_since: earlier is later than self"),
            ));
        }
        return Ok(Duration::from_millis((self.ms - earlier.ms) as u64));
    }

    /// The time elapsed between earlier and this reading, or None when
    /// earlier is later than this reading.
    pub fn checked_duration_since(&self, earlier: SystemTime) -> Option<Duration> {
        if self.ms < earlier.ms {
            return None;
        }
        return Some(Duration::from_millis((self.ms - earlier.ms) as u64));
    }
}

/// The Unix epoch.
///
/// Rust spells this UNIX_EPOCH as a constant; a lsaca const cannot have a
/// struct type, so 2.1.0 provides it as a function with the same value.
pub fn unix_epoch() -> SystemTime {
    return SystemTime::from_unix_ms(0);
}

/// Suspends the current thread for the given duration.
pub fn sleep(duration: Duration) {
    let milliseconds: u64 = duration.as_millis();
    if milliseconds > 9223372036854775807u64 {
        __time_sleep_ms(9223372036854775807);
        return;
    }
    __time_sleep_ms(milliseconds as i64);
}

// ===========================================================================
// Compatibility helpers
// ===========================================================================

/// Milliseconds since the Unix epoch.
///
/// Deprecated in 2.1.0: use SystemTime::now() and its methods.
pub fn now_ms() -> i64 {
    return __time_now_ms();
}

/// A monotonic nanosecond counter for measuring durations.
///
/// Deprecated in 2.1.0: use Instant::now() and its methods.
pub fn monotonic_ns() -> i64 {
    return __time_monotonic_ns();
}

/// Suspends the current thread for the given number of milliseconds.
///
/// Deprecated in 2.1.0: use sleep(Duration).
pub fn sleep_ms(milliseconds: i64) {
    __time_sleep_ms(milliseconds);
}

/// A calendar date and time broken into its parts.
///
/// weekday: 0 Sunday ... 6 Saturday; yearday: 0 based day of the year.
///
/// Deprecated in 2.1.0 for new code: keep wall clock values as SystemTime and
/// convert only for display.
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

impl DateTime {
    /// The current calendar time in the local time zone.
    pub fn now_local() -> Result<DateTime, Error> {
        let parts = __time_local_parts(__time_now_ms());
        let (ok, year, month, day, hour, minute, second, weekday, yearday) = parts;
        if ok != 0 {
            return Err(Error::last());
        }
        return Ok(DateTime {
            year: year,
            month: month,
            day: day,
            hour: hour,
            minute: minute,
            second: second,
            weekday: weekday,
            yearday: yearday,
        });
    }

    /// The current calendar time in UTC.
    pub fn now_utc() -> Result<DateTime, Error> {
        let parts = __time_utc_parts(__time_now_ms());
        let (ok, year, month, day, hour, minute, second, weekday, yearday) = parts;
        if ok != 0 {
            return Err(Error::last());
        }
        return Ok(DateTime {
            year: year,
            month: month,
            day: day,
            hour: hour,
            minute: minute,
            second: second,
            weekday: weekday,
            yearday: yearday,
        });
    }

    /// The calendar year.
    pub fn year(&self) -> i64 { return self.year; }
    /// The month, 1 through 12.
    pub fn month(&self) -> i64 { return self.month; }
    /// The day of the month, 1 through 31.
    pub fn day(&self) -> i64 { return self.day; }
    /// The hour, 0 through 23.
    pub fn hour(&self) -> i64 { return self.hour; }
    /// The minute, 0 through 59.
    pub fn minute(&self) -> i64 { return self.minute; }
    /// The second, 0 through 59.
    pub fn second(&self) -> i64 { return self.second; }
    /// The weekday, 0 (Sunday) through 6 (Saturday).
    pub fn weekday(&self) -> i64 { return self.weekday; }
    /// The zero based day of the year.
    pub fn yearday(&self) -> i64 { return self.yearday; }

    /// Formats the date as YYYY-MM-DD.
    pub fn format_date(&self) -> String {
        return format!("{:04}-{:02}-{:02}", self.year, self.month, self.day);
    }

    /// Formats the time of day as HH:MM:SS.
    pub fn format_time(&self) -> String {
        return format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second);
    }

    /// Formats the date and time as YYYY-MM-DD HH:MM:SS.
    pub fn format_datetime(&self) -> String {
        return format!("{} {}", self.format_date(), self.format_time());
    }

    /// The English name of the weekday.
    pub fn weekday_name(&self) -> String {
        match self.weekday {
            0 => { return String::from("Sunday"); }
            1 => { return String::from("Monday"); }
            2 => { return String::from("Tuesday"); }
            3 => { return String::from("Wednesday"); }
            4 => { return String::from("Thursday"); }
            5 => { return String::from("Friday"); }
            6 => { return String::from("Saturday"); }
            _ => { return String::from("Unknown"); }
        }
    }
}
