// audit: a public type alias of another module can be used in annotations.
mod types;

fn main() {
    let length: types::Length = types::Meter { value: 5 };
    println!("{}", length.value);
}
