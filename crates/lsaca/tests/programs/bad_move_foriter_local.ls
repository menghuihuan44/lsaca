// A local moved inside a for body is moved at the loop head on the next
// iteration, so the second iteration is E0382.
fn main() {
    let numbers: Vec<i64> = vec![1, 2, 3];
    let text = String::from("x");
    for value in numbers.iter() {
        let moved = text;
        println!("{} {}", value, moved);
    }
}
