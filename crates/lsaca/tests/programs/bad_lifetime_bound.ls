// Lifetime parameters cannot carry bounds (E0001).
struct View<'a: 'static> {
    text: &'a str,
}

fn main() {
}
