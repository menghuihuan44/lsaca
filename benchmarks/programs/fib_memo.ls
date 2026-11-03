// fib_memo: recursive fibonacci with a memo table - another implementation of
// the fib specification (same checksum). Optional arguments: n [repetitions];
// the defaults are the medium scale.
struct FibTable {
    values: Vec<i64>,
}

impl FibTable {
    fn new(size: i64) -> FibTable {
        let mut values: Vec<i64> = Vec::new();
        let mut i: i64 = 0;
        while i < size {
            values.push(-1);
            i = i + 1;
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
    let all = args();
    let n: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 30 };
    let reps: i64 = if all.len() > 1 { all[1].clone().parse_i64().unwrap() } else { 10 };
    let mut table = FibTable::new(n + 1);
    let mut checksum: i64 = 0;
    let mut r: i64 = 0;
    while r < reps {
        checksum = checksum + table.compute(n);
        r = r + 1;
    }
    println!("checksum={}", checksum);
}