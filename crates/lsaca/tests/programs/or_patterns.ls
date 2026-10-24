// Or-patterns test every alternative: the first arm whose pattern matches
// (and whose guard accepts) wins, not merely the first arm.
fn label(v: i64) -> String {
    match v {
        0 | 1 => String::from("small"),
        2 | 3 | 4 => String::from("medium"),
        _ => String::from("large"),
    }
}

fn main() {
    println!("{}", label(0));
    println!("{}", label(3));
    println!("{}", label(9));
    let x = 1;
    let y = 2;
    match x {
        0 | 1 => println!("a"),
        2 | 3 => println!("b"),
        _ => println!("c"),
    }
    match y {
        0 | 1 => println!("p"),
        2 | 3 => println!("q"),
        _ => println!("r"),
    }
}
