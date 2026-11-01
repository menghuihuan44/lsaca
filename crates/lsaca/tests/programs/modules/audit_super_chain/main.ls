// audit: super and super::super walk up the module tree.
mod a;

fn main() {
    println!("{}", a::b::c::total());
}
