fn main() {
    let from: String = String::from("fs_extra_from.txt");
    let to: String = String::from("fs_extra_to.txt");
    let written = std::fs::write(&from, "abc");
    println!("write ok {}", written.is_ok());
    let appended = std::fs::append(&from, "def");
    println!("append ok {}", appended.is_ok());
    match std::fs::read_to_string(&from) {
        Ok(text) => { println!("read after append {}", text); }
        Err(error) => { println!("read after append failed"); }
    }
    let copied = std::fs::copy(&from, &to);
    println!("copy ok {}", copied.is_ok());
    match std::fs::read_to_string(&to) {
        Ok(text) => { println!("read copy {}", text); }
        Err(error) => { println!("read copy failed"); }
    }
    let canonical = std::fs::canonicalize(&from);
    println!("canonicalize ok {}", canonical.is_ok());
    let cwd = std::fs::current_dir();
    println!("current dir ok {}", cwd.is_ok());
    let temp = std::fs::temp_dir();
    println!("temp dir non empty {}", temp.len() > 0);
    let bytes = std::fs::read(&from);
    match bytes {
        Ok(data) => { println!("read bytes {}", data.len()); }
        Err(error) => { println!("read bytes failed"); }
    }
    println!("remove from ok {}", std::fs::remove_file(&from).is_ok());
    println!("remove to ok {}", std::fs::remove_file(&to).is_ok());
    println!("exists after remove {}", std::fs::exists(&from));
}