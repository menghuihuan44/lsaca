fn main() {
    let mut lines = 0;
    let mut words = 0;
    loop {
        if let Some(line) = read_line() {
            lines = lines + 1;
            words = words + line.split(" ").len();
        } else {
            break;
        }
    }
    println!("{} lines, {} words", lines, words);
}
