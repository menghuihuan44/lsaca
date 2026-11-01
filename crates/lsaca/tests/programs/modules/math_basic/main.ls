mod math;
use math::{add, multiply as mul};

fn main() {
    let sum = add(2, 3);
    let product = mul(sum, 4);
    println!("{} {}", sum, product);
}
