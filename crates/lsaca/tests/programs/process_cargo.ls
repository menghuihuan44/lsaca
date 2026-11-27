use std::process::{Command, ExitStatus, Output};

fn main() {
    let mut command = Command::new(String::from_str("cargo"));
    command.arg(String::from_str("--version"));
    match command.output() {
        Ok(output) => {
            let status: ExitStatus = output.status();
            let stdout: String = output.stdout();
            let stderr: String = output.stderr();
            println!("code {}", status.code());
            println!("success {}", status.success());
            println!("stdout nonempty {}", !stdout.is_empty());
            println!("stderr empty {}", stderr.is_empty());
        }
        Err(error) => {
            println!("error {}", error.message());
        }
    }
}
