fn arg_i64(args: &[String], index: usize, fallback: i64) -> i64 {
    if index < args.len() {
        args[index].parse::<i64>().unwrap()
    } else {
        fallback
    }
}

fn main() {
    // Optional argument: token count; the default is the medium scale.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tokens = arg_i64(&args, 0, 1_000_000);
    let mut s: String = String::with_capacity((tokens * 4) as usize);
    let mut token_sum: i64 = 0;
    for i in 0..tokens {
        let v = i % 1000;
        token_sum += v;
        s.push_str(&v.to_string());
        s.push(',');
    }
    println!("checksum={},{}", s.len(), token_sum);
}
