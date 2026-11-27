use std::io::ErrorKind;
use std::process::Command;

fn main() {
    let mut command = Command::new(String::from_str("lsaca-program-that-does-not-exist"));
    match command.status() {
        Ok(status) => {
            println!("unexpected {}", status.code());
        }
        Err(error) => {
            match error.kind() {
                ErrorKind::NotFound => {
                    println!("NotFound");
                }
                ErrorKind::PermissionDenied => {
                    println!("PermissionDenied");
                }
                _ => {
                    println!("Other");
                }
            }
        }
    }
}
