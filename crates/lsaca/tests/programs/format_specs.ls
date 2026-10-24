// Format specifiers follow Rust's grammar:
// [[fill]align][sign][0][width][.precision][?].
fn main() {
    let name = String::from("lsaca");
    println!("[{}]", name);
    println!("[{:>8}]", name);
    println!("[{:<8}]", name);
    println!("[{:^8}]", name);
    println!("[{:*>8}]", name);
    println!("[{:08.2}]", 3.14159);
    println!("[{:+}] [{:+}] [{:+}]", 42, -7, 0);
    println!("[{:8}] [{:8}]", 3.5, 2.0);
    println!("[{:.2}] [{:.0}]", 8.0 / 3.0, 2.5);
    println!("[{:.3}]", 1.0 / 3.0);
    println!("[{:.2}]", String::from("\u{4f60}\u{597d}\u{4e16}\u{754c}"));
    println!("[{:>8.3}]", 3.0);
    println!("[{:?}]", name);
    println!("[{:>6?}]", String::from("hi"));
    println!("[{:04}]", 5);
    println!("[{:12.3}]", 1234.5678);
    println!("[{:^20}]", 7);
    println!("[{:*^9}]", true);
    let message = format!("{:>5}", 42);
    println!("[{}]", message);
}
