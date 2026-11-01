// audit: a for loop over a vector returned by another module consumes it.
mod lists;

fn main() {
    let mut total = 0;
    for number in lists::nums() {
        total = total + number;
    }
    println!("{}", total);
}
