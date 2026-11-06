// Exercises the system-facing standard library: time, files, and the
// environment. The harness runs this program in a private scratch working
// directory, so file system operations use paths inside that directory (both
// relative and absolute) and never depend on the system temporary directory
// being writable by child processes (some sandboxed environments deny that).

fn main() {
    let start: Instant = Instant::now();
    sleep(Duration::from_millis(5));
    println!("{}", start.elapsed().as_millis() >= 5);
    println!("{} {}", Duration::from_secs(1).as_secs(), Duration::new(1, 500000000).subsec_nanos());
    println!("{}", Duration::from_millis(100).checked_add(Duration::from_millis(50)).unwrap().as_millis());
    println!("{} {}", SystemTime::now().unix_ms() > 0, unix_epoch().unix_ms());
    println!("{}", fs::temp_dir().len() > 0);
    let path: String = String::from("lsaca_system_std.txt");
    println!("{}", fs::write(&path, "hello").is_ok());
    println!("{}", fs::read_to_string(&path).unwrap());
    let metadata = fs::metadata(&path).unwrap();
    println!("{}", metadata.size());
    match metadata.file_type() {
        FileType::File => println!("file"),
        FileType::Dir => println!("dir"),
        FileType::Other => println!("other"),
    }
    println!("{}", fs::remove_file(&path).is_ok());
    let base = fs::current_dir().unwrap();
    let absolute: String = base + "/lsaca_absolute.txt";
    println!("{}", fs::write(&absolute, "absolute").is_ok());
    println!("{}", fs::read_to_string(&absolute).unwrap());
    println!("{}", fs::remove_file(&absolute).is_ok());
    println!("{} {}", env::var("LSACA_E2E_PRESENT").is_ok(), env::var("LSACA_SURELY_MISSING_VAR").is_err());
}
