struct Pair {
    left: i64,
    right: i64,
}

fn main() {
    let mut pair = Pair { left: 1, right: 2 };
    let left = &mut pair.left;
    *left = 10;
    let right = &mut pair.right;
    *right = 20;
    println!("{} {}", pair.left, pair.right);

    let first = &pair.left;
    let total = *first + pair.right;
    println!("{}", total);
}
