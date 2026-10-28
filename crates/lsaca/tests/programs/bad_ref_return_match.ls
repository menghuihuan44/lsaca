fn choose(flag: bool, other: &i64) -> &i64 {
    match flag {
        true => other,
        false => {
            let x = 5;
            &x
        }
    }
}

fn main() {
    let value = 7;
    let r = choose(false, &value);
    println!("{}", *r);
}
