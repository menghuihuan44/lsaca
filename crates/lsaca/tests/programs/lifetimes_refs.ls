struct View<'a> {
    text: &'a str,
}

impl<'a> View<'a> {
    fn size(&self) -> usize {
        self.text.len()
    }

    fn starts(&self, prefix: &str) -> bool {
        self.text.starts_with(prefix)
    }
}

fn wide(view: &View) -> String {
    let mut out = String::from("<");
    out.push_str(view.text);
    out.push_str(">");
    out
}

fn main() {
    let owned: String = String::from("hello, lifetimes");
    let view: View = View { text: owned.as_str() };
    println!("{}", view.text);
    println!("{}", view.size());
    println!("{}", view.starts("hello"));
    println!("{}", wide(&view));
}
