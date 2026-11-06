fn main() {
    let path = std::env::var("PATH");
    println!("path present {}", path.is_ok());
    let missing = std::env::var("LSACA_TASK69_DEFINITELY_UNSET_VARIABLE");
    println!("missing absent {}", missing.is_err());
    let variables = std::env::vars();
    println!("vars count positive {}", variables.len() > 0);
    let arguments = std::process::args();
    println!("args count {}", arguments.len());
}
