fn main() {
    let mut squares: Vec<i64> = Vec::new();
    for i in 0..10 {
        squares.push(i * i);
    }

    let mut sum = 0;
    for square in squares {
        sum = sum + square;
    }
    println!("sum of squares: {}", sum);

    let first = vec![10, 20, 30].first();
    match first {
        Some(value) => println!("first = {}", value),
        None => println!("no first value"),
    }
}
