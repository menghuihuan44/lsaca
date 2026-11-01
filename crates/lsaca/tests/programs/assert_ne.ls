fn main() {
    assert_ne!(1, 2);
    assert_ne!("left", "right");
    assert_ne!(3, 4, "values {} and {}", 3, 4);
    assert_ne!(true, false);
    println!("ok");
}
