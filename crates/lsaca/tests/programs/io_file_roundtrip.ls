fn main() {
    let path: String = String::from("io_fixture_scratch.txt");
    let created = std::io::File::create(&path);
    if created.is_err() {
        println!("create failed");
        return;
    }
    let mut file = created.unwrap();
    let mut data: Vec<u8> = Vec::new();
    data.push(104);
    data.push(105);
    data.push(33);
    let written = file.write(data.as_slice());
    if written.is_err() {
        println!("write failed");
        return;
    }
    println!("written {}", written.unwrap());
    println!("flush ok {}", file.flush().is_ok());
    let size = file.size();
    println!("size {}", size.unwrap());
    println!("close ok {}", file.close().is_ok());
    let opened = std::io::File::open(&path);
    if opened.is_err() {
        println!("open failed");
        return;
    }
    let mut reader = opened.unwrap();
    let seeked = reader.seek(0, 0);
    println!("seek {}", seeked.unwrap());
    let mut buffer: Vec<u8> = Vec::new();
    while buffer.len() < 8 {
        buffer.push(0);
    }
    let count = reader.read(buffer.as_mut_slice()).unwrap();
    println!("read {}", count);
    data.clear();
    let mut index: usize = 0;
    while index < count {
        data.push(buffer.get(index).unwrap());
        index = index + 1;
    }
    println!("bytes {:?}", data);
    println!("close ok {}", reader.close().is_ok());
    match std::fs::remove_file(&path) {
        Ok(unit) => { println!("removed"); }
        Err(error) => { println!("remove failed"); }
    }
}