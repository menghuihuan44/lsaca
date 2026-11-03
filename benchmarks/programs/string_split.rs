// string_split: build a comma-separated string of decimal numbers, split it,
// parse every field and sum it.
// Optional argument: token count (default 50,000, the medium scale).
fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tokens = arg_i64(&args, 0, 50_000);
    let mut text = String::new();
    for i in 0..tokens {
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
