// Conditions, loop conditions and match guards all require `bool`.
// Numeric values are rejected even when their type is only inferred.
fn main() {
    let flag: i64 = 1;
    if flag {
        println!("truthy");
    }
    let count = 3;
    while count {
        println!("loop");
    }
    let n = 4;
    match n {
        n if n => println!("guard"),
        _ => println!("none"),
    }
}
