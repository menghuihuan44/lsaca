fn main() {
    let now = std::time::now_ms();
    println!("now positive {}", now > 0);
    let local_result = std::time::DateTime::now_local();
    if local_result.is_err() {
        println!("local failed");
        return;
    }
    let local = local_result.unwrap();
    println!("year range {}", local.year() >= 2024 && local.year() < 2100);
    println!("month range {}", local.month() >= 1 && local.month() <= 12);
    println!("day range {}", local.day() >= 1 && local.day() <= 31);
    println!("hour range {}", local.hour() >= 0 && local.hour() <= 23);
    println!("weekday range {}", local.weekday() >= 0 && local.weekday() <= 6);
    println!("date len {}", local.format_date().len() == 10);
    println!("time len {}", local.format_time().len() == 8);
    println!("datetime len {}", local.format_datetime().len() == 19);
    println!("weekday name non empty {}", local.weekday_name().len() > 0);
    let utc_result = std::time::DateTime::now_utc();
    if utc_result.is_err() {
        println!("utc failed");
        return;
    }
    let utc = utc_result.unwrap();
    println!("utc year range {}", utc.year() >= 2024 && utc.year() < 2100);
    let first = std::time::monotonic_ns();
    std::time::sleep_ms(5);
    let second = std::time::monotonic_ns();
    println!("monotonic grows {}", second > first);
}
