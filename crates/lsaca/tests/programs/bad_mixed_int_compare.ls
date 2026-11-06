// Comparison requires both operands to have the same integer type (v2.1).
fn main() {
    let index: i32 = 1;
    let values = vec![10, 20, 30];
    if index < values.len() {
        println!("in range");
    }
}
