fn main() {
    let tokens: i64 = 1000000;
    let mut s = String::new();
    let mut token_sum: i64 = 0;
    let mut i: i64 = 0;
    while i < tokens {
        let v = i % 1000;
        token_sum += v;
        s.push_str(format!("{},", v));
        i += 1;
    }
    println!("checksum={},{}", s.len(), token_sum);
}
