// Moving a String into a new binding leaves the source unusable (E0382).
fn main() {
    let name = String::from("lsaca");
    let moved = name;
    println!("{}", moved);
    println!("{}", name);
}
