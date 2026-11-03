fn main() {
    let local_result = std::time::DateTime::now_local();
    if local_result.is_err() {
        println!("local failed");
        return;
    }
    let local = local_result.unwrap();
    let date = local.format_date();
    let time = local.format_time();
    println!("date dashes {} {}", date.as_str().char_at(4).unwrap() == '-', date.as_str().char_at(7).unwrap() == '-');
    println!("date digits {} {}", date.as_str().char_at(0).unwrap() != '-', date.as_str().char_at(9).unwrap() != '-');
    println!("time colons {} {}", time.as_str().char_at(2).unwrap() == ':', time.as_str().char_at(5).unwrap() == ':');
    let stamp = local.format_datetime();
    println!("stamp len {}", stamp.len() == 19);
    println!("stamp separator {}", stamp.as_str().char_at(10).unwrap() == ' ');
    println!("weekday name {}", local.weekday_name().len() > 0);
}