// string_split: build a comma-separated string of 50,000 decimal numbers
// (about 194,500 bytes), split it, parse every field and sum it.
const TOKENS: i64 = 50000;

fn main() {
    let mut text = String::new();
    for i in 0..TOKENS {
        text.push_str(&(i % 1000).to_string());
        text.push(',');
    }

    let mut count = 0i64;
    let mut sum = 0i64;
    for part in text.split(',') {
        if !part.is_empty() {
            sum += part.parse::<i64>().unwrap();
            count += 1;
        }
    }
    println!("checksum={},{},{}", text.len(), count, sum);
}
