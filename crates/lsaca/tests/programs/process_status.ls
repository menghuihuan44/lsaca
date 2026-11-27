use std::process::{Command, ExitStatus, Output};

fn main() {
    let mut command = Command::new(String::from_str("cargo"));
    command.arg(String::from_str("--lsaca-invalid-flag"));
    match command.output() {
        Ok(output) => {
            let status: ExitStatus = output.status();
            println!("code nonzero {}", status.code() != 0);
            println!("success {}", status.success());
        }
        Err(error) => {
            println!("error {}", error.message());
        }
    }
}
