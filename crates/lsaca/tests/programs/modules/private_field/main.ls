mod counter;

fn main() {
    let counter = counter::Counter::new(3);
    println!("{}", counter.value);
}
