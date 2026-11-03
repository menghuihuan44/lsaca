// Time helpers.
//
// Wall clock values are Unix milliseconds; the monotonic clock is a
// nanosecond counter suitable for measuring durations. Calendar parts are
// available in local time and in UTC. Failures are reported as io::Error
// values, never as panics.

pub extern fn __time_now_ms() -> i64;
pub extern fn __time_monotonic_ns() -> i64;
pub extern fn __time_sleep_ms(milliseconds: i64);
pub extern fn __time_local_parts(unix_ms: i64) -> (i64, i64, i64, i64, i64, i64, i64, i64, i64);
pub extern fn __time_utc_parts(unix_ms: i64) -> (i64, i64, i64, i64, i64, i64, i64, i64, i64);

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    return __time_now_ms();
}

/// A monotonic nanosecond counter for measuring durations.
pub fn monotonic_ns() -> i64 {
    return __time_monotonic_ns();
}

/// Suspends the current thread for the given number of milliseconds.
pub fn sleep_ms(milliseconds: i64) {
    __time_sleep_ms(milliseconds);
}

/// A calendar date and time broken into its parts.
///
/// weekday: 0 Sunday ... 6 Saturday; yearday: 0 based day of the year.
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
        let mut text: String = String::new();
        text.push_str(DateTime::pad(self.year, 4));
        text.push('-');
        text.push_str(DateTime::pad(self.month, 2));
        text.push('-');
        text.push_str(DateTime::pad(self.day, 2));
        return text;
    }

    /// Formats the time of day as HH:MM:SS.
    pub fn format_time(&self) -> String {
        let mut text: String = String::new();
        text.push_str(DateTime::pad(self.hour, 2));
        text.push(':');
        text.push_str(DateTime::pad(self.minute, 2));
        text.push(':');
        text.push_str(DateTime::pad(self.second, 2));
        return text;
    }

    /// Formats the date and time as YYYY-MM-DD HH:MM:SS.
    pub fn format_datetime(&self) -> String {
        let mut text: String = String::new();
        text.push_str(self.format_date());
        text.push(' ');
        text.push_str(self.format_time());
        return text;
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

    fn pad(value: i64, width: i64) -> String {
        let mut digits: Vec<i64> = Vec::new();
        let mut remaining: i64 = value;
        if remaining < 0 {
            remaining = 0 - remaining;
        }
        if remaining == 0 {
            digits.push(0);
        }
        while remaining > 0 {
            digits.push(remaining % 10);
            remaining = remaining / 10;
        }
        let mut text: String = String::new();
        let mut position: i64 = width - 1;
        while position >= 0 {
            let mut digit: i64 = 0;
            if position < digits.len() {
                digit = digits.get(position).unwrap();
            }
            text.push(DateTime::digit_char(digit));
            position = position - 1;
        }
        return text;
    }

    fn digit_char(digit: i64) -> char {
        match digit {
            0 => { return '0'; }
            1 => { return '1'; }
            2 => { return '2'; }
            3 => { return '3'; }
            4 => { return '4'; }
            5 => { return '5'; }
            6 => { return '6'; }
            7 => { return '7'; }
            8 => { return '8'; }
            9 => { return '9'; }
            _ => { return '?'; }
        }
    }
}
