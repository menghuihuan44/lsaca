// Regression: function values used to be invalid C when stored in a struct
// field or a Vec.
struct Handler {
    name: String,
    apply: fn(i64) -> i64,
}

fn double(value: i64) -> i64 {
    return value * 2;
}

fn triple(value: i64) -> i64 {
    return value * 3;
}

fn main() {
    let mut handlers: Vec<Handler> = Vec::new();
    handlers.push(Handler { name: String::from("double"), apply: double });
    handlers.push(Handler { name: String::from("triple"), apply: triple });
    for handler in handlers {
        println!("{} {}", handler.name, (handler.apply)(5));
    }

    let mut functions: Vec<fn(i64) -> i64> = Vec::new();
    functions.push(double);
    functions.push(triple);
    let mut total = 0;
    for function in functions {
        total = total + (function)(2);
    }
    println!("{}", total);
}
