fn id(value: &i64) -> &i64 {
    value
}

fn choose(flag: bool, other: &i64) -> &i64 {
    id(if flag {
        other
    } else {
        let x = 5;
        &x
    })
}

fn main() {
    let value = 7;
    let r = choose(false, &value);
    println!("{}", *r);
}
