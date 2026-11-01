use greeting::secret;

fn main() {
    let hidden = secret();
    println!("{}", hidden.hidden);
}
