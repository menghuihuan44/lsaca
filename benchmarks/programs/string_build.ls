fn main() {
    // Optional argument: token count; the default is the medium scale.
    let all = args();
    let tokens: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 1000000 };
    let mut s = String::new();
    let mut token_sum: i64 = 0;
    let mut i: i64 = 0;
    while i < tokens {
        let v = i % 1000;
        token_sum += v;
        let piece = format!("{},", v);
        s.push_str(&piece);
        i += 1;
    }
    println!("checksum={},{}", s.len(), token_sum);
}
