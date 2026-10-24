fn main() {
    let tokens: i64 = 1_000_000;
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
