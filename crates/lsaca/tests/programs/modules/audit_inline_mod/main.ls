// audit: an inline module can declare an external child module.
mod outer {
    mod inner;

    pub fn via() -> i64 {
        inner::value()
    }
}

fn main() {
    println!("{}", outer::via());
}
