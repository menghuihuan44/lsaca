// Match guards: an arm is taken only when its pattern matches and its guard
// evaluates to true; a false guard continues with the remaining arms.
enum Msg {
    Text(String),
    Num(i64),
}

fn classify(msg: Msg) -> i64 {
    match msg {
        Msg::Text(s) if s == "hi" => 1,
        Msg::Text(_) => 2,
        Msg::Num(n) if n > 5 => 3,
        Msg::Num(_) => 4,
    }
}

fn main() {
    println!("{}", classify(Msg::Text(String::from("hi"))));
    println!("{}", classify(Msg::Text(String::from("yo"))));
    println!("{}", classify(Msg::Num(9)));
    println!("{}", classify(Msg::Num(2)));

    let v = 5;
    match v {
        n if n > 10 => println!("big"),
        n if n > 0 => println!("small"),
        _ => println!("zero"),
    }
    let w = 0;
    match w {
        n if n > 10 => println!("big"),
        n if n > 0 => println!("small"),
        _ => println!("zero"),
    }
}
