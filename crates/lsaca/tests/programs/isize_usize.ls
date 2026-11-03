fn twice(value: isize) -> isize {
    return value * 2;
}

fn main() {
    let a: isize = -5;
    let b: usize = 7;
    let c: isize = a + 3;
    let d: usize = b * 2;
    println!("{} {} {} {}", a, b, c, d);
    let e: usize = 42 as usize;
    let f: isize = e as isize;
    let g: isize = twice(f) + 1;
    println!("{} {} {}", e, f, g);
}
