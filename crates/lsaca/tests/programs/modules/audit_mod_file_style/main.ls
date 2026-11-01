// audit: a module may also live in a directory as mod.ls.
mod tree;

fn main() {
    println!("{}", tree::value());
}
