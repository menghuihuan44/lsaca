fn main() {
    let name = "world";
    let greeting = format!("hello, {}!", name);
    println!("{}", greeting);
    println!("{}", greeting.len());
    let s = String::from("foo");
    let t = s + String::from("bar");
    println!("{} {}", t, s);
    println!("{}", t.contains("oob"));
    println!("{}", t.to_uppercase());
    println!("{:?}", t);
    println!("line1\nline2\ttab\\done");
}
