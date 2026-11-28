extern "stdcall" {
    fn puts(text: *const u8) -> i32;
}

fn main() {
    println!("unreachable");
}
