// pub is accepted on items, struct fields and enum variants and is ignored:
// every item of a single-file program is visible.
pub struct Config {
    pub name: String,
    pub retries: i64,
}

pub enum Mode {
    Fast,
    Slow,
}

pub fn make(name: String) -> Config {
    return Config { name: name, retries: 2 };
}

fn main() {
    let config = make(String::from("main"));
    println!("{} {}", config.name, config.retries);
    let mode = Mode::Fast;
    match mode {
        Mode::Fast => println!("fast"),
        Mode::Slow => println!("slow"),
    }
}
