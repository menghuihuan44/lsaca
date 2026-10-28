fn main() {
    let mut items: Vec<i64> = Vec::new();
    items.push(3);
    items.push(1);
    items.push(2);
    items.insert(0, 9);
    println!("{}", items.len());
    println!("{}", items[0]);
    println!("{}", items.pop().unwrap());
    items.reverse();
    for item in items.clone() {
        println!("{}", item);
    }
    println!("{}", items.len());
}
