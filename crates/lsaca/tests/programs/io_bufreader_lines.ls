// BufReader::read_line and the BufRead read_to_string helper work over both a
// concrete File and a boxed Read trait object.
use std::io::{BufRead, BufReader, File, Read};

fn main() {
    let path: String = String::from("io_bufreader_lines.txt");
    match std::fs::write(&path, "alpha\nbeta\n") {
        Ok(unit) => {}
        Err(error) => {
            println!("write failed");
            return;
        }
    }
    let first = File::open(&path);
    if first.is_err() {
        println!("open failed");
        return;
    }
    {
        let mut reader: BufReader<File> = BufReader::new(first.unwrap());
        loop {
            let mut line: String = String::new();
            let result = reader.read_line(&mut line);
            match result {
                Ok(0) => {
                    break;
                }
                Ok(count) => {
                    let text: &str = line.trim();
                    println!("line {} {}", count, text);
                }
                Err(error) => {
                    println!("read_line failed");
                    break;
                }
            }
        }
    }
    let second = File::open(&path);
    if second.is_err() {
        println!("open failed");
        return;
    }
    {
        let mut reader: BufReader<Box<dyn Read>> =
            BufReader::new(Box::new(second.unwrap()));
        let mut all: String = String::new();
        match reader.read_to_string(&mut all) {
            Ok(count) => {
                println!("all bytes {}", count);
                println!("all text {}", all.trim());
            }
            Err(error) => {
                println!("read_to_string failed");
            }
        }
    }
    println!("removed {}", std::fs::remove_file(&path).is_ok());
}
