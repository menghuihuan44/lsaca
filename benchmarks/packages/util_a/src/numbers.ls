// numbers: integer helpers for the util_a library package.
//
// Every function is deterministic and independent of hash-map iteration
// order, so the package checksum is stable across compilers and platforms.

pub const MODULUS: i64 = 1000000007;

pub enum Sign {
    Negative,
    Zero,
    Positive,
}

pub fn sign_of(value: i64) -> Sign {
    if value < 0 {
        return Sign::Negative;
    }
    if value == 0 {
        return Sign::Zero;
    }
    return Sign::Positive;
}

pub fn sign_code(value: i64) -> i64 {
    match sign_of(value) {
        Sign::Negative => {
            return -1;
        }
        Sign::Zero => {
            return 0;
        }
        Sign::Positive => {
            return 1;
        }
    }
}

pub struct Span {
    pub start: i64,
    pub length: i64,
}

impl Span {
    pub fn new(start: i64, length: i64) -> Span {
        return Span { start: start, length: length };
    }

    pub fn end(&self) -> i64 {
        return self.start + self.length;
    }

    pub fn contains(&self, value: i64) -> bool {
        if value < self.start {
            return false;
        }
        return value < self.end();
    }

    pub fn overlap(&self, other: &Span) -> i64 {
        let mut lower = self.start;
        if other.start > lower {
            lower = other.start;
        }
        let mut upper = self.end();
        if other.end() < upper {
            upper = other.end();
        }
        if upper > lower {
            return upper - lower;
        }
        return 0;
    }

    pub fn checksum(&self) -> i64 {
        return (self.start * 31 + self.length) % MODULUS;
    }
}

pub fn gcd(a: i64, b: i64) -> i64 {
    let mut x = a;
    let mut y = b;
    if x < 0 {
        x = -x;
    }
    if y < 0 {
        y = -y;
    }
    while y != 0 {
        let next = x % y;
        x = y;
        y = next;
    }
    return x;
}

pub fn lcm(a: i64, b: i64) -> i64 {
    let mut x = a;
    if x < 0 {
        x = -x;
    }
    let mut y = b;
    if y < 0 {
        y = -y;
    }
    let base = gcd(x, y);
    if base == 0 {
        return 0;
    }
    return (x / base) * y;
}

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
        divisor = divisor + 2;
    }
    return true;
}

pub fn primes_upto(limit: i64) -> Vec<i64> {
    let mut values: Vec<i64> = Vec::new();
    let mut candidate = 2;
    while candidate <= limit {
        if is_prime(candidate) {
            values.push(candidate);
        }
        candidate = candidate + 1;
    }
    return values;
}

pub fn prime_count(limit: i64) -> i64 {
    let mut count = 0;
    let mut candidate = 2;
    while candidate <= limit {
        if is_prime(candidate) {
            count = count + 1;
        }
        candidate = candidate + 1;
    }
    return count;
}

pub fn digit_total(value: i64) -> i64 {
    let mut rest = value;
    if rest < 0 {
        rest = -rest;
    }
    let mut total = 0;
    while rest > 0 {
        total = total + rest % 10;
        rest = rest / 10;
    }
    return total;
}

pub fn reverse_digits(value: i64) -> i64 {
    let mut rest = value;
    if rest < 0 {
        rest = -rest;
    }
    let mut reversed = 0;
    while rest > 0 {
        reversed = reversed * 10 + rest % 10;
        rest = rest / 10;
    }
    return reversed;
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
        steps = steps + 1;
    }
    return steps;
}

pub fn power_mod(base: i64, exponent: i64, modulus: i64) -> i64 {
    let mut result = 1 % modulus;
    let mut factor = base % modulus;
    let mut power = exponent;
    while power > 0 {
        if power % 2 == 1 {
            result = (result * factor) % modulus;
        }
        factor = (factor * factor) % modulus;
        power = power / 2;
    }
    return result;
}

pub fn triangular(n: i64) -> i64 {
    return n * (n + 1) / 2;
}

pub fn sum_squares(n: i64) -> i64 {
    let mut total = 0;
    let mut value = 1;
    while value <= n {
        total = total + value * value;
        value = value + 1;
    }
    return total;
}

pub fn factorial_mod(n: i64, modulus: i64) -> i64 {
    let mut result = 1;
    let mut value = 2;
    while value <= n {
        result = (result * value) % modulus;
        value = value + 1;
    }
    return result;
}

pub fn gcd_of_slice(values: Vec<i64>) -> i64 {
    let mut current = 0;
    let mut index = 0;
    let len = values.len();
    while index < len {
        current = gcd(current, values[index]);
        index = index + 1;
    }
    return current;
}

pub fn lcm_of_slice(values: Vec<i64>) -> i64 {
    let mut current = 1;
    let mut index = 0;
    let len = values.len();
    while index < len {
        current = lcm(current, values[index]);
        index = index + 1;
    }
    return current;
}

pub fn next_square(value: i64) -> i64 {
    let mut root = 0;
    while root * root < value {
        root = root + 1;
    }
    return root * root;
}

pub fn digit_histogram(value: i64) -> Vec<i64> {
    let mut counts: Vec<i64> = Vec::new();
    let mut index = 0;
    while index < 10 {
        counts.push(0);
        index = index + 1;
    }
    let mut rest = value;
    if rest < 0 {
        rest = -rest;
    }
    if rest == 0 {
        counts[0] = counts[0] + 1;
    }
    while rest > 0 {
        let digit = rest % 10;
        counts[digit as usize] = counts[digit as usize] + 1;
        rest = rest / 10;
    }
    return counts;
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;
    acc = (acc * 31 + gcd(1071, 462)) % MODULUS;
    acc = (acc * 31 + lcm(21, 6)) % MODULUS;
    acc = (acc * 31 + sign_code(-5)) % MODULUS;
    acc = (acc * 31 + digit_total(987654321)) % MODULUS;
    acc = (acc * 31 + power_mod(3, 13, 1000003)) % MODULUS;
    acc = (acc * 31 + prime_count(300)) % MODULUS;
    acc = (acc * 31 + triangular(100)) % MODULUS;
    acc = (acc * 31 + sum_squares(20)) % MODULUS;
    acc = (acc * 31 + factorial_mod(10, 1000003)) % MODULUS;
    acc = (acc * 31 + collatz_steps(27)) % MODULUS;
    acc = (acc * 31 + reverse_digits(120345)) % MODULUS;
    let left = Span::new(3, 7);
    let right = Span::new(8, 5);
    acc = (acc * 31 + left.overlap(&right)) % MODULUS;
    acc = (acc * 31 + left.checksum()) % MODULUS;
    let primes = primes_upto(100);
    acc = (acc * 31 + gcd_of_slice(primes)) % MODULUS;
    let widths: Vec<i64> = vec![12, 18, 24];
    acc = (acc * 31 + lcm_of_slice(widths)) % MODULUS;
    acc = (acc * 31 + next_square(50)) % MODULUS;
    let histogram = digit_histogram(987654321);
    let mut index = 0;
    while index < histogram.len() {
        acc = (acc * 31 + histogram[index]) % MODULUS;
        index = index + 1;
    }
    return acc;
}
