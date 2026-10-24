fn main() {
    let numbers = vec![10, 20, 30];
    println!("{}", numbers.get(1).unwrap());
    println!("{}", numbers.get(9).unwrap_or(-1));
    let parsed: Result<i64, String> = "42".parse_i64();
    println!("{}", parsed.unwrap());
    let bad = "oops".parse_i64();
    println!("{}", bad.is_err());
    let maybe: Option<i64> = None;
    println!("{}", maybe.unwrap_or(7));
}
