union Bad {
    text: String,
}

fn main() {
    let bad = unsafe { Bad { text: String::from("managed") } };
    println!("unreachable");
}
