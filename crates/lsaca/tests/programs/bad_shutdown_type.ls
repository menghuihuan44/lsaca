// net::Shutdown is an enum value, not an integer (v2.1).
use std::net::Shutdown;

fn shut(how: Shutdown) {
    println!("shutting down");
}

fn main() {
    shut(0);
}
