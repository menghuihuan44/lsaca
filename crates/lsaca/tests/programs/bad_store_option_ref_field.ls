// A struct field cannot store Option<&T> (E0498).
struct Maybe {
    value: Option<&i64>,
}

fn main() {
    let number = 1;
    let maybe = Maybe { value: Some(&number) };
    match maybe.value {
        Some(value) => println!("{}", *value),
        None => {}
    }
}
