// stats: aggregate helpers for the util_b library package.

use util_a::gcd;

pub const MODULUS: i64 = 1000000007;

pub struct Stats {
    pub sum: i64,
    pub count: i64,
    pub min: i64,
    pub max: i64,
}

impl Stats {
    pub fn new() -> Stats {
        return Stats { sum: 0, count: 0, min: 0, max: 0 };
    }

    pub fn observe(&mut self, value: i64) {
        self.sum = self.sum + value;
        if self.count == 0 {
            self.min = value;
            self.max = value;
        } else {
            if value < self.min {
                self.min = value;
            }
            if value > self.max {
                self.max = value;
            }
        }
        self.count = self.count + 1;
    }

    pub fn mean(&self) -> i64 {
        if self.count == 0 {
            return 0;
        }
        return self.sum / self.count;
    }

    pub fn range(&self) -> i64 {
        if self.count == 0 {
            return 0;
        }
        return self.max - self.min;
    }

    pub fn checksum(&self) -> i64 {
        let mut state = (self.sum * 31 + self.count * 17) % MODULUS;
        state = (state + self.min * 13 + self.max * 7) % MODULUS;
        return state;
    }
}

pub fn total(values: Vec<i64>) -> i64 {
    let mut sum = 0;
    for value in values {
        sum = sum + value;
    }
    return sum;
}

pub fn every_other_sum(values: Vec<i64>) -> i64 {
    let mut sum = 0;
    let mut index = 0;
    let len = values.len();
    while index < len {
        if index % 2 == 0 {
            sum = sum + values[index];
        }
        index = index + 1;
    }
    return sum;
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;
    let mut stats = Stats::new();
    stats.observe(4);
    stats.observe(9);
    stats.observe(2);
    stats.observe(7);
    stats.observe(5);
    acc = (acc * 31 + stats.mean()) % MODULUS;
    acc = (acc * 31 + stats.range()) % MODULUS;
    acc = (acc * 31 + stats.checksum()) % MODULUS;
    let mut values: Vec<i64> = Vec::new();
    values.push(3);
    values.push(1);
    values.push(4);
    values.push(1);
    values.push(5);
    values.push(9);
    values.push(2);
    values.push(6);
    acc = (acc * 31 + total(values.clone())) % MODULUS;
    acc = (acc * 31 + every_other_sum(values)) % MODULUS;
    acc = (acc * 31 + gcd(84, 36)) % MODULUS;
    return acc;
}
