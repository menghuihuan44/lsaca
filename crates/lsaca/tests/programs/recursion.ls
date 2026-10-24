fn factorial(n: i64) -> i64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

fn fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn is_even(n: i64) -> bool {
    if n == 0 {
        true
    } else {
        is_odd(n - 1)
    }
}

fn is_odd(n: i64) -> bool {
    if n == 0 {
        false
    } else {
        is_even(n - 1)
    }
}

fn main() {
    println!("{}", factorial(5));
    println!("{}", fib(10));
    println!("{} {}", is_even(10), is_odd(7));
}
