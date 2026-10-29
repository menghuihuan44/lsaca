// Regression: nested tuples and nested tuple patterns used to generate
// invalid C. Tuples are now fully recursive.
fn main() {
    let nested = ((1, 2), (3, 4));
    println!("{:?}", nested);
    let ((a, b), (c, d)) = nested;
    println!("{} {} {} {}", a, b, c, d);
    let triple = (1, (2, 3));
    println!("{:?}", triple);
    let (first, (inner, last)) = triple;
    println!("{} {}", first, inner);
    println!("{}", last);
}
