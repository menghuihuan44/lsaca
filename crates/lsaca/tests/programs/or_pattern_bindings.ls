// Or-pattern alternatives may bind the same names; the variables are bound
// from the alternative that actually matched.
enum E {
    Pair(i64, String),
    Single(String),
}

struct P {
    x: i64,
    y: i64,
}

fn text(e: E) -> String {
    match e {
        E::Pair(_, s) | E::Single(s) => s,
    }
}

fn main() {
    println!("{}", text(E::Pair(1, String::from("pair"))));
    println!("{}", text(E::Single(String::from("single"))));

    let p = P { x: 2, y: 9 };
    match p {
        P { x: 0 | 1, y } => println!("low {}", y),
        P { x: 2 | 3, y } => println!("mid {}", y),
        _ => println!("other"),
    }

    let pair = (3, 4);
    match pair {
        (1 | 2, b) => println!("a {}", b),
        (3, b) => println!("b {}", b),
        _ => println!("c"),
    }
}
