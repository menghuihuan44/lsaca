// arithmetic: integer helpers used by the modules_compile compile-time workload.
//
// This module is part of a deliberately large multi-module program: the crate
// root (main.ls) declares eight modules and folds each module checksum into a
// single printed value. The program is built but never executed by the benchmark
// harness, so the goal of the code is to exercise a broad slice of the language
// (structs, enums, generics, impls, loops, matching, integer arithmetic) through
// many separate module files.

pub const MODULUS: i64 = 1000000007;
pub const PRIME_LIMIT: i64 = 1000;

// parity: which half of the integers a value belongs to.
pub enum Parity {
    Even,
    Odd,
}

pub fn parity_is_even(parity: Parity) -> bool {
    match parity {
        Parity::Even => true,
        Parity::Odd => false,
    }
}

pub fn parity_as_i64(parity: Parity) -> i64 {
    match parity {
        Parity::Even => 0,
        Parity::Odd => 1,
    }
}

pub fn parity_of(value: i64) -> Parity {
    if value % 2 == 0 {
        Parity::Even
    } else {
        Parity::Odd
    }
}

// gcd: Euclid's algorithm on non-negative values.
pub fn gcd(left: i64, right: i64) -> i64 {
    let mut a = left;
    let mut b = right;
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a
}

pub fn lcm(left: i64, right: i64) -> i64 {
    if left == 0 || right == 0 {
        return 0;
    }
    let divisor = gcd(left, right);
    (left / divisor) * right
}

pub fn gcd_of_vec(values: &Vec<i64>) -> i64 {
    let mut result = 0;
    let mut index = 0;
    while index < values.len() {
        result = gcd(result, values[index]);
        index += 1;
    }
    result
}

pub fn lcm_of_vec(values: &Vec<i64>) -> i64 {
    let mut result = 1;
    let mut index = 0;
    while index < values.len() {
        result = lcm(result, values[index]);
        index += 1;
    }
    result
}

// is_prime: trial division up to the integer square root.
pub fn is_prime(value: i64) -> bool {
    if value < 2 {
        return false;
    }
    if value < 4 {
        return true;
    }
    if value % 2 == 0 {
        return false;
    }
    let mut divisor = 3;
    while divisor * divisor <= value {
        if value % divisor == 0 {
            return false;
        }
        divisor += 2;
    }
    true
}

pub fn primes_upto(limit: i64) -> Vec<i64> {
    let mut primes: Vec<i64> = Vec::new();
    let mut candidate = 2;
    while candidate <= limit {
        if is_prime(candidate) {
            primes.push(candidate);
        }
        candidate += 1;
    }
    primes
}

pub fn prime_count(limit: i64) -> i64 {
    let primes = primes_upto(limit);
    primes.len() as i64
}

// mod_pow: base ** exponent modulo modulus, using square-and-multiply.
pub fn mod_pow(base: i64, exponent: i64, modulus: i64) -> i64 {
    let mut result = 1;
    let mut factor = base % modulus;
    let mut power = exponent;
    while power > 0 {
        if power % 2 == 1 {
            result = (result * factor) % modulus;
        }
        factor = (factor * factor) % modulus;
        power = power / 2;
    }
    result
}

pub fn digit_sum(value: i64) -> i64 {
    let mut rest = value;
    let mut total = 0;
    while rest > 0 {
        total += rest % 10;
        rest = rest / 10;
    }
    total
}

pub fn digit_count(value: i64) -> i64 {
    if value == 0 {
        return 1;
    }
    let mut rest = value;
    let mut count = 0;
    while rest > 0 {
        count += 1;
        rest = rest / 10;
    }
    count
}

pub fn reverse_number(value: i64) -> i64 {
    let mut rest = value;
    let mut reversed = 0;
    while rest > 0 {
        reversed = reversed * 10 + rest % 10;
        rest = rest / 10;
    }
    reversed
}

pub fn is_number_palindrome(value: i64) -> bool {
    if value < 0 {
        return false;
    }
    value == reverse_number(value)
}

pub fn factorial_mod(limit: i64, modulus: i64) -> i64 {
    let mut result = 1;
    let mut value = 2;
    while value <= limit {
        result = (result * value) % modulus;
        value += 1;
    }
    result
}

pub fn binomial_mod(n: i64, k: i64, modulus: i64) -> i64 {
    if k < 0 || k > n {
        return 0;
    }
    let mut numerator = 1;
    let mut denominator = 1;
    let mut step = 0;
    while step < k {
        numerator = (numerator * (n - step)) % modulus;
        denominator = (denominator * (step + 1)) % modulus;
        step += 1;
    }
    let inverse = mod_pow(denominator, modulus - 2, modulus);
    (numerator * inverse) % modulus
}

pub fn triangular(limit: i64) -> i64 {
    if limit <= 0 {
        return 0;
    }
    (limit * (limit + 1)) / 2
}

pub fn collatz_steps(start: i64) -> i64 {
    let mut value = start;
    let mut steps = 0;
    while value != 1 && steps < 1000 {
        if value % 2 == 0 {
            value = value / 2;
        } else {
            value = value * 3 + 1;
        }
        steps += 1;
    }
    steps
}

pub fn collatz_peak(start: i64) -> i64 {
    let mut value = start;
    let mut peak = start;
    let mut steps = 0;
    while value != 1 && steps < 1000 {
        if value > peak {
            peak = value;
        }
        if value % 2 == 0 {
            value = value / 2;
        } else {
            value = value * 3 + 1;
        }
        steps += 1;
    }
    peak
}

pub fn next_power_of_two(value: i64) -> i64 {
    if value <= 1 {
        return 1;
    }
    let mut result = 1;
    while result < value {
        result = result * 2;
    }
    result
}

pub fn floor_sqrt(value: i64) -> i64 {
    if value <= 0 {
        return 0;
    }
    let mut guess = 1;
    while (guess + 1) * (guess + 1) <= value {
        guess += 1;
    }
    guess
}

pub fn fibonacci_iter(count: i64) -> i64 {
    let mut previous = 0;
    let mut current = 1;
    let mut index = 0;
    while index < count {
        let next = previous + current;
        previous = current;
        current = next;
        index += 1;
    }
    previous
}

pub fn fibonacci_mod(count: i64, modulus: i64) -> i64 {
    let mut previous = 0;
    let mut current = 1;
    let mut index = 0;
    while index < count {
        let next = (previous + current) % modulus;
        previous = current;
        current = next;
        index += 1;
    }
    previous
}

// FactorSet: an owned collection of divisors with running aggregates.
pub struct FactorSet {
    numbers: Vec<i64>,
    total: i64,
}

impl FactorSet {
    pub fn new() -> FactorSet {
        FactorSet {
            numbers: Vec::new(),
            total: 0,
        }
    }

    pub fn add(&mut self, value: i64) {
        self.numbers.push(value);
        self.total += value;
    }

    pub fn len(&self) -> i64 {
        self.numbers.len() as i64
    }

    pub fn total(&self) -> i64 {
        self.total
    }

    pub fn largest(&self) -> i64 {
        let mut result = 0;
        let mut index = 0;
        while index < self.numbers.len() {
            let number = self.numbers[index];
            if number > result {
                result = number;
            }
            index += 1;
        }
        result
    }

    pub fn smallest(&self) -> i64 {
        if self.numbers.len() == 0 {
            return 0;
        }
        let mut result = self.numbers[0];
        let mut index = 1;
        while index < self.numbers.len() {
            if self.numbers[index] < result {
                result = self.numbers[index];
            }
            index += 1;
        }
        result
    }

    pub fn product_mod(&self, modulus: i64) -> i64 {
        let mut result = 1;
        let mut index = 0;
        while index < self.numbers.len() {
            result = (result * self.numbers[index]) % modulus;
            index += 1;
        }
        result
    }

    pub fn even_count(&self) -> i64 {
        let mut count = 0;
        let mut index = 0;
        while index < self.numbers.len() {
            if self.numbers[index] % 2 == 0 {
                count += 1;
            }
            index += 1;
        }
        count
    }

    pub fn summary(&self) -> String {
        format!(
            "count={} total={} min={} max={}",
            self.len(),
            self.total(),
            self.smallest(),
            self.largest()
        )
    }

    pub fn divisors_of(value: i64) -> FactorSet {
        let mut set = FactorSet::new();
        let mut divisor = 1;
        while divisor <= value {
            if value % divisor == 0 {
                set.add(divisor);
            }
            divisor += 1;
        }
        set
    }
}

// Range: a half-open integer interval with a few derived queries.
pub struct Range {
    start: i64,
    end: i64,
}

impl Range {
    pub fn new(start: i64, end: i64) -> Range {
        Range { start: start, end: end }
    }

    pub fn len(&self) -> i64 {
        if self.end <= self.start {
            return 0;
        }
        self.end - self.start
    }

    pub fn sum(&self) -> i64 {
        if self.len() == 0 {
            return 0;
        }
        let mut total = 0;
        let mut value = self.start;
        while value < self.end {
            total += value;
            value += 1;
        }
        total
    }

    pub fn evens(&self) -> Vec<i64> {
        let mut result: Vec<i64> = Vec::new();
        let mut value = self.start;
        while value < self.end {
            if value % 2 == 0 {
                result.push(value);
            }
            value += 1;
        }
        result
    }

    pub fn contains(&self, value: i64) -> bool {
        value >= self.start && value < self.end
    }
}

// is_even is a tiny convenience wrapper used by the checksum section.
pub fn is_even(value: i64) -> bool {
    parity_is_even(parity_of(value))
}

pub fn odd_index_sum(values: &Vec<i64>) -> i64 {
    let mut total = 0;
    let mut index = 1;
    while index < values.len() {
        total += values[index];
        index += 2;
    }
    total
}

pub fn digit_histogram(value: i64) -> Vec<i64> {
    let mut counts: Vec<i64> = Vec::new();
    let mut digit = 0;
    while digit < 10 {
        counts.push(0);
        digit += 1;
    }
    let mut rest = value;
    if rest == 0 {
        counts[0] = counts[0] + 1;
    }
    while rest > 0 {
        let index = rest % 10;
        counts[index as usize] = counts[index as usize] + 1;
        rest = rest / 10;
    }
    counts
}

pub fn are_coprime(left: i64, right: i64) -> bool {
    gcd(left, right) == 1
}

pub fn euler_totient(value: i64) -> i64 {
    if value <= 1 {
        return 0;
    }
    let mut reduced = value;
    let mut result = value;
    let mut candidate = 2;
    while candidate * candidate <= reduced {
        if reduced % candidate == 0 {
            while reduced % candidate == 0 {
                reduced = reduced / candidate;
            }
            result = result - result / candidate;
        }
        candidate += 1;
    }
    if reduced > 1 {
        result = result - result / reduced;
    }
    result
}

pub fn sum_of_squares(limit: i64) -> i64 {
    let mut total = 0;
    let mut value = 1;
    while value <= limit {
        total += value * value;
        value += 1;
    }
    total
}

pub fn sum_of_cubes(limit: i64) -> i64 {
    let mut total = 0;
    let mut value = 1;
    while value <= limit {
        total += value * value * value;
        value += 1;
    }
    total
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;
    acc = (acc * 31 + gcd(1071, 462)) % MODULUS;
    acc = (acc * 31 + lcm(21, 6)) % MODULUS;
    acc = (acc * 31 + digit_sum(987654321)) % MODULUS;
    acc = (acc * 31 + mod_pow(3, 13, 1000003)) % MODULUS;
    acc = (acc * 31 + prime_count(200)) % MODULUS;
    acc = (acc * 31 + collatz_steps(27)) % MODULUS;
    acc = (acc * 31 + triangular(100)) % MODULUS;
    acc = (acc * 31 + reverse_number(120345)) % MODULUS;
    acc % MODULUS
}
