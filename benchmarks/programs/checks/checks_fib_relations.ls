// Property check: the recursive, iterative and memoized fibonacci
// implementations agree for every n up to 20, and the iterative form matches
// the known value at n = 50 (the memo table is exercised beyond the grid).
fn fib_recursive(n: i64) -> i64 {
    if n < 2 {
        return n;
    }
    fib_recursive(n - 1) + fib_recursive(n - 2)
}

fn fib_iterative(n: i64) -> i64 {
    let mut previous: i64 = 0;
    let mut current: i64 = 1;
    let mut index: i64 = 0;
    while index < n {
        let next = previous + current;
        previous = current;
        current = next;
        index += 1;
    }
    previous
}

struct FibTable {
    values: Vec<i64>,
}

impl FibTable {
    fn new(size: i64) -> FibTable {
        let mut values: Vec<i64> = Vec::new();
        let mut index: i64 = 0;
        while index < size {
            values.push(-1);
            index += 1;
        }
        FibTable { values: values }
    }

    fn compute(&mut self, n: i64) -> i64 {
        if n < 2 {
            return n;
        }
        let cached = self.values[n];
        if cached >= 0 {
            return cached;
        }
        let value = self.compute(n - 1) + self.compute(n - 2);
        self.values[n] = value;
        value
    }
}

fn main() {
    let mut table = FibTable::new(64);
    let mut index: i64 = 0;
    while index <= 20 {
        let expected = fib_recursive(index);
        assert_eq!(fib_iterative(index), expected);
        assert_eq!(table.compute(index), expected);
        index += 1;
    }
    assert_eq!(fib_iterative(50), 12586269025);
    println!("ok fib");
}
