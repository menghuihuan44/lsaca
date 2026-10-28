struct Holder {
    value: &i64,
}

fn main() {
    let number = 1;
    let holder = Holder { value: &number };
    println!("{}", *holder.value);
}
