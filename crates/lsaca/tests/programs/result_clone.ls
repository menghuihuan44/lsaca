fn compute(flag: bool) -> Result<i64, String> {
    if flag {
        Ok(21)
    } else {
        Err(String::from("missing"))
    }
}

fn main() {
    let good = compute(true);
    println!("{}", good.clone().unwrap());
    println!("{}", good.clone().unwrap_or(0));
    println!("{}", good.unwrap());

    let bad = compute(false);
    println!("{}", bad.clone().unwrap_or(-1));
    println!("{}", bad.is_err());
}
