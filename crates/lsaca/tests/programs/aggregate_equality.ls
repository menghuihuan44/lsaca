// Regression: == for aggregates (Vec, Option, Result, tuples, structs,
// enums, references, Box, HashMap) and assert_eq! accepting them.
struct Point {
    x: i64,
    y: i64,
}

enum Kind {
    Empty,
    Value(i64),
}

fn main() {
    println!("{} {}", vec![1, 2] == vec![1, 2], vec![1, 2] == vec![2, 1]);
    println!("{} {}", Some(3) == Some(3), Some(3) == None);
    // A Result left unconstrained (Ok(...) == Ok(...)) has no inferable error
    // type, so pin it with annotations.
    let ok_one: Result<i64, i64> = Ok(1);
    let ok_two: Result<i64, i64> = Ok(1);
    let failed: Result<i64, i64> = Err(2);
    println!("{} {}", ok_one == ok_two, ok_one == failed);
    println!("{} {}", (1, true) == (1, true), (1, true) == (2, true));
    println!("{} {}", Point { x: 1, y: 2 } == Point { x: 1, y: 2 }, Point { x: 1, y: 2 } == Point { x: 0, y: 2 });
    println!("{} {}", Kind::Empty == Kind::Empty, Kind::Value(3) == Kind::Value(4));
    let left = String::from("same");
    let right = String::from("same");
    println!("{}", &left == &right);
    assert_eq!(Box::new(5), Box::new(5));
    let mut first: HashMap<String, i64> = HashMap::new();
    first.insert(String::from("k"), 1);
    let mut second: HashMap<String, i64> = HashMap::new();
    second.insert(String::from("k"), 1);
    println!("{}", first == second);
    println!("equality ok");
}
