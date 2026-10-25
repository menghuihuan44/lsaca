// string_split: build a comma-separated string of 50,000 decimal numbers
// (about 194,500 bytes), split it, parse every field and sum it.
fn main() {
    let tokens: i64 = 50000;
    let mut text = String::new();
    let mut i = 0;
    while i < tokens {
        text.push_str(format!("{},", i % 1000));
        i += 1;
    }

    let parts = text.split(",");
    let mut count = 0;
    let mut sum = 0;
    for part in parts {
        if part.len() > 0 {
            sum += part.parse_i64().unwrap();
            count += 1;
        }
    }
    println!("checksum={},{},{}", text.len(), count, sum);
}
