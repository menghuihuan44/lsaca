fn main() {
    let path = std::env::var(String::from("PATH"));
    println!("path present {}", path.is_some());
    let missing = std::env::var(String::from("LSACA_TASK69_DEFINITELY_UNSET_VARIABLE"));
    println!("missing absent {}", missing.is_none());
    let variables = std::env::vars();
    println!("vars count positive {}", variables.len() > 0);
    let arguments = std::process::args();
    println!("args count {}", arguments.len());
}
