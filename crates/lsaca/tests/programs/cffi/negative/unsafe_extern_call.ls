extern "C" {
    fn strlen(text: *const u8) -> usize;
}

fn main() {
    let empty: *const u8 = 0 as *const u8;
    let length: usize = strlen(empty);
    println!("{}", length);
}
