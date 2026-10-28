fn main() {
    let value = 10;
    let a = &value;
    let b = a;
    println!("{} {}", *a, *b);

    let mut n = 1;
    let m = &mut n;
    *m = *m + 41;
    println!("{}", n);

    let y = 5;
    let p = &y;
    let pp = &p;
    println!("{}", **pp);

    let r = &*p;
    println!("{}", *r);
}
