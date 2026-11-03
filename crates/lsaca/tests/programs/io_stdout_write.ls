fn main() {
    let mut stdout = std::io::stdout();
    let text: String = String::from("raw line\n");
    let written = stdout.write(text.as_str().as_bytes());
    println!("wrote {}", written.unwrap());
    let stdin_handle = std::io::stdin().handle();
    let stderr_handle = std::io::stderr().handle();
    println!("handles {} {} {}", stdin_handle, stdout.handle(), stderr_handle);
}
