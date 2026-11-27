// read_dir_entries yields DirEntry values with a path, a name and a file type.
// The tree is created by the program and removed before it exits.
use std::fs;
use std::fs::{PathBuf, read_dir_entries};

fn main() {
    let root: String = String::from("read_dir_tree");
    let root_path: PathBuf = PathBuf::from_str(root.as_str());
    match fs::create_dir_all(root_path.as_str()) {
        Ok(unit) => {}
        Err(error) => {
            println!("create_dir_all failed");
            return;
        }
    }
    let mut first: PathBuf = root_path.join("a.txt");
    match fs::write(first.as_str(), "one") {
        Ok(unit) => {}
        Err(error) => {
            println!("write failed");
            return;
        }
    }
    let mut second: PathBuf = root_path.join("b.txt");
    match fs::write(second.as_str(), "two") {
        Ok(unit) => {}
        Err(error) => {
            println!("write failed");
            return;
        }
    }
    let mut nested: PathBuf = root_path.join("sub");
    match fs::create_dir(nested.as_str()) {
        Ok(unit) => {}
        Err(error) => {
            println!("create_dir failed");
            return;
        }
    }
    let mut inner: PathBuf = nested.join("inner.txt");
    match fs::write(inner.as_str(), "inner") {
        Ok(unit) => {}
        Err(error) => {
            println!("write failed");
            return;
        }
    }
    println!("exists {} is_dir {} is_file {}", root_path.exists(),
             root_path.is_dir(), root_path.is_file());
    println!("file exists {}", first.exists());
    let listed = read_dir_entries(root_path.as_str());
    if listed.is_err() {
        println!("read_dir failed");
        return;
    }
    let mut entries = listed.unwrap();
    for entry in entries {
        let kind = entry.file_type();
        let label: String = if kind.is_file() {
            String::from_str("file")
        } else if kind.is_dir() {
            String::from_str("dir")
        } else {
            String::from_str("other")
        };
        println!("entry {} {} {}", entry.file_name(), label,
                 entry.path().display());
    }
    println!("removed {}", fs::remove_dir_all(root_path.as_str()).is_ok());
}
