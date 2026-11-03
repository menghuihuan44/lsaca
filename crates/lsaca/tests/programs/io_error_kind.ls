fn main() {
    let missing: String = String::from("io_fixture_missing.txt");
    let opened = std::io::File::open(missing.clone());
    println!("missing open is_err {}", opened.is_err());
    let kind = std::io::Error::last().kind();
    match kind {
        std::io::ErrorKind::NotFound => { println!("kind NotFound"); }
        std::io::ErrorKind::PermissionDenied => { println!("kind PermissionDenied"); }
        std::io::ErrorKind::Other => { println!("kind Other"); }
        _ => { println!("kind unexpected"); }
    }
    let message = std::io::Error::last().message();
    println!("message non empty {}", message.len() > 0);
    let out = std::io::stdout();
    println!("stdout handle {}", out.handle());
}
