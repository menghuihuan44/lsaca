// Partial moves: a destructuring pattern that binds an owning value moves the
// whole initializer out of the source binding.

struct Point {
    x: i64,
    y: i64,
}

struct Payload {
    name: String,
    values: Vec<i64>,
    id: i64,
}

fn main() {
    let payload = Payload {
        name: String::from("alpha"),
        values: vec![1, 2, 3],
        id: 7,
    };
    let Payload { name, values, id } = payload;
    println!("{} {} {}", name, values.len(), id);

    // The fields that are not bound are released with the moved temporary.
    let rest = Payload {
        name: String::from("beta"),
        values: vec![4, 5],
        id: 8,
    };
    let Payload { name, .. } = rest;
    println!("{}", name);

    let pair = (String::from("gamma"), 42);
    let (text, number) = pair;
    println!("{} {}", text, number);

    // Destructuring a temporary moves the temporary and drops what is left.
    let (tmp_text, tmp_number) = (String::from("delta"), 5);
    println!("{} {}", tmp_text, tmp_number);

    // A pattern that only binds Copy values still copies: the source stays live.
    let copy_pair = (1, 2);
    let (first, second) = copy_pair;
    println!("{} {} {} {}", first, second, copy_pair.0, copy_pair.1);

    let origin = Point { x: 10, y: 20 };
    let Point { x: left, y: right } = origin;
    println!("{} {} {}", left, right, origin.x);
}