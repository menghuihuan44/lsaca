// Path and PathBuf operate lexically on path text. No file system state is
// needed here, so the output does not depend on the machine.
use std::fs::{Path, PathBuf};

fn main() {
    let mut assets: PathBuf = PathBuf::from_str("assets");
    println!("start {}", assets.display());
    assets.push("icons");
    assets.push("logo.png");
    println!("joined {}", assets.display());
    match assets.parent() {
        Some(parent) => {
            println!("parent {}", parent.display());
        }
        None => {
            println!("parent none");
        }
    }
    match assets.file_name() {
        Some(name) => {
            println!("name {}", name);
        }
        None => {
            println!("name none");
        }
    }
    match assets.extension() {
        Some(extension) => {
            println!("extension {}", extension);
        }
        None => {
            println!("extension none");
        }
    }
    println!("exists {} is_file {} is_dir {}", assets.exists(),
             assets.is_file(), assets.is_dir());
    let nested: PathBuf = assets.join("nested");
    println!("join {}", nested.display());
    println!("pop {} {}", assets.pop(), assets.display());
    println!("to_string {}", assets.to_string());
    println!("as_str {}", assets.as_str());
    let view: Path = Path::new("docs/readme.md");
    println!("view {}", view.display());
    match view.parent() {
        Some(parent) => {
            println!("view parent {}", parent.display());
        }
        None => {
            println!("view parent none");
        }
    }
}
