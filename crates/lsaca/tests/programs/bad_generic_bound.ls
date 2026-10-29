// A generic bound must be satisfied at instantiation (E0277).
trait Sized2 {
    fn size(&self) -> i64;
}

fn measure<T: Sized2>(value: &T) -> i64 {
    return value.size();
}

fn main() {
    let number = 3;
    println!("{}", measure(&number));
}
