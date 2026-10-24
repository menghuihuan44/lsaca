fn main() {
    let x: i64 = 300;
    let y = x as i8;
    println!("{}", y);
    let f = 3.99;
    let n = f as i64;
    println!("{}", n);
    let b = true;
    println!("{}", b as i64);
    println!("{}", 65 as u8 as i64);
}
