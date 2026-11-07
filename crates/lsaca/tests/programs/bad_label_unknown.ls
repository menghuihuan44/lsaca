// A break label must name an enclosing loop (E0426).
fn main() {
    'outer: loop {
        break 'missing;
    }
}
