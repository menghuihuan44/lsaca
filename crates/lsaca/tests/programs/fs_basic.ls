// File system content helpers work on owned String paths and report
// failures as io::Error values instead of panicking. The harness runs this
// program with a scratch working directory.

fn main() {
    let path: String = String::from("fs_basic_scratch.txt");
    match std::fs::write(path.clone(), String::from("hello file")) {
        Ok(unit) => {
            println!("write ok");
        }
        Err(error) => {
            println!("write failed");
        }
    }
    match std::fs::read_to_string(path.clone()) {
        Ok(text) => {
            println!("read: {}", text);
        }
        Err(error) => {
            println!("read failed");
        }
    }
    match std::fs::read_to_string(String::from("fs_basic_missing.txt")) {
        Ok(text) => {
            println!("unexpected {}", text);
        }
        Err(error) => {
            println!("missing failed");
        }
    }
}
