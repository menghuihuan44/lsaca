fn main() {
    let dir: String = String::from("fs_fixture_dir");
    std::fs::remove_dir_all(dir.clone());
    let made = std::fs::create_dir_all(dir.clone());
    println!("create_dir_all ok {}", made.is_ok());
    let mut file_path: String = dir.clone();
    file_path.push_str("/payload.txt");
    match std::fs::write(file_path.clone(), String::from("hello metadata")) {
        Ok(unit) => { println!("write ok"); }
        Err(error) => { println!("write failed"); }
    }
    match std::fs::read_to_string(file_path.clone()) {
        Ok(text) => { println!("read {}", text); }
        Err(error) => { println!("read failed"); }
    }
    let info = std::fs::metadata(file_path.clone());
    if info.is_err() {
        println!("metadata failed");
        return;
    }
    let metadata = info.unwrap();
    println!("size {}", metadata.size());
    println!("is_file {} is_dir {}", metadata.is_file(), metadata.is_dir());
    println!("kind {}", metadata.kind());
    let dir_info = std::fs::metadata(dir.clone()).unwrap();
    println!("dir is_dir {}", dir_info.is_dir());
    println!("exists {} is_file {} is_dir {}", std::fs::exists(file_path.clone()), std::fs::is_file(file_path.clone()), std::fs::is_dir(dir.clone()));
    println!("missing {}", std::fs::exists(String::from("fs_fixture_missing.txt")));
    let entries_result = std::fs::read_dir(dir.clone());
    if entries_result.is_err() {
        println!("read_dir failed");
        return;
    }
    let entries = entries_result.unwrap();
    println!("entries {:?}", entries);
    println!("remove_file ok {}", std::fs::remove_file(file_path.clone()).is_ok());
    println!("remove_dir_all ok {}", std::fs::remove_dir_all(dir.clone()).is_ok());
}
