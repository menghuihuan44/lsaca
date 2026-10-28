fn side(counter: &mut i64) -> bool {
    *counter = *counter + 1;
    println!("side");
    true
}

fn main() {
    let mut counter = 0;
    let a = true;
    let b = false;
    println!("{}", a || b);
    println!("{}", a && b);
    println!("{}", b || a);
    println!("{}", b && a);
    let c = true || side(&mut counter);
    println!("{} counter = {}", c, counter);
    let d = false && side(&mut counter);
    println!("{} counter = {}", d, counter);
    if false || true {
        println!("branch");
    }
}
