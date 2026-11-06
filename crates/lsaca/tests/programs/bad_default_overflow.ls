// Integer literals default to i32, so an unconstrained literal must fit (v2.1).
fn main() {
    let value = 3000000000;
    println!("{}", value);
}
