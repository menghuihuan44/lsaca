fn main() {
    let dir: String = String::from("fs_err_dir");
    std::fs::remove_dir_all(dir.clone());
    let first = std::fs::create_dir(dir.clone());
    println!("first create ok {}", first.is_ok());
    let second = std::fs::create_dir(dir.clone());
    println!("second create is_err {}", second.is_err());
    let kind = std::io::Error::last().kind();
    match kind {
        std::io::ErrorKind::AlreadyExists => { println!("second create kind AlreadyExists true"); }
        std::io::ErrorKind::Other => { println!("second create kind Other true"); }
        _ => { println!("second create kind unexpected"); }
    }
    println!("remove dir ok {}", std::fs::remove_dir(dir.clone()).is_ok());
    let missing: String = String::from("fs_err_missing.txt");
    let removed = std::fs::remove_file(missing.clone());
    println!("missing remove is_err {}", removed.is_err());
    let missing_kind = std::io::Error::last().kind();
    match missing_kind {
        std::io::ErrorKind::NotFound => { println!("missing remove kind NotFound true"); }
        std::io::ErrorKind::Other => { println!("missing remove kind Other true"); }
        _ => { println!("missing remove kind unexpected"); }
    }
}