// vectors: Vec<i64> algorithms and small container types built on top of Vec.
//
// Part of the modules_compile compile-time workload. The crate root folds this
// module's checksum together with seven other module checksums. Every function
// here works on owned Vec values or on &Vec references with explicit index
// loops, which is the style the language supports in this version.

pub const MODULUS: i64 = 1000000007;

// sum_vec: total of all elements.
pub fn sum_vec(values: &Vec<i64>) -> i64 {
    let mut total = 0;
    let mut index = 0;
    while index < values.len() {
        total += values[index];
        index += 1;
    }
    total
}

// product_mod: product of all elements modulo a caller-supplied modulus.
pub fn product_mod(values: &Vec<i64>, modulus: i64) -> i64 {
    let mut result = 1;
    let mut index = 0;
    while index < values.len() {
        result = (result * values[index]) % modulus;
        index += 1;
    }
    result
}

pub fn max_value(values: &Vec<i64>) -> i64 {
    if values.len() == 0 {
        return 0;
    }
    let mut result = values[0];
    let mut index = 1;
    while index < values.len() {
        if values[index] > result {
            result = values[index];
        }
        index += 1;
    }
    result
}

pub fn min_value(values: &Vec<i64>) -> i64 {
    if values.len() == 0 {
        return 0;
    }
    let mut result = values[0];
    let mut index = 1;
    while index < values.len() {
        if values[index] < result {
            result = values[index];
        }
        index += 1;
    }
    result
}

pub fn count_positive(values: &Vec<i64>) -> i64 {
    let mut count = 0;
    let mut index = 0;
    while index < values.len() {
        if values[index] > 0 {
            count += 1;
        }
        index += 1;
    }
    count
}

pub fn count_even(values: &Vec<i64>) -> i64 {
    let mut count = 0;
    let mut index = 0;
    while index < values.len() {
        if values[index] % 2 == 0 {
            count += 1;
        }
        index += 1;
    }
    count
}

pub fn count_above(values: &Vec<i64>, limit: i64) -> i64 {
    let mut count = 0;
    let mut index = 0;
    while index < values.len() {
        if values[index] > limit {
            count += 1;
        }
        index += 1;
    }
    count
}

pub fn first_index_of(values: &Vec<i64>, wanted: i64) -> i64 {
    let mut index = 0;
    while index < values.len() {
        if values[index] == wanted {
            return index as i64;
        }
        index += 1;
    }
    return -1;
}

pub fn last_index_of(values: &Vec<i64>, wanted: i64) -> i64 {
    let mut index = values.len() - 1;
    while index >= 0 {
        if values[index] == wanted {
            return index as i64;
        }
        index -= 1;
    }
    return -1;
}

// binary_search: index of target in a sorted ascending Vec, or -1.
pub fn binary_search(sorted: &Vec<i64>, target: i64) -> i64 {
    let mut low = 0;
    let mut high = sorted.len() - 1;
    while low <= high {
        let middle = low + (high - low) / 2;
        let value = sorted[middle];
        if value == target {
            return middle as i64;
        }
        if value < target {
            low = middle + 1;
        } else {
            high = middle - 1;
        }
    }
    return -1;
}

pub fn is_sorted(values: &Vec<i64>) -> bool {
    let mut index = 1;
    while index < values.len() {
        if values[index - 1] > values[index] {
            return false;
        }
        index += 1;
    }
    true
}

// reverse_in_place: swap symmetric elements.
pub fn reverse_in_place(values: &mut Vec<i64>) {
    let mut left = 0;
    let mut right = values.len() - 1;
    while left < right {
        let temporary = values[left];
        values[left] = values[right];
        values[right] = temporary;
        left += 1;
        right -= 1;
    }
}

// bubble_sort: simple quadratic sort, adequate for short vectors.
pub fn bubble_sort(values: &mut Vec<i64>) {
    let mut end = values.len();
    while end > 1 {
        let mut index = 0;
        while index + 1 < end {
            if values[index] > values[index + 1] {
                let temporary = values[index];
                values[index] = values[index + 1];
                values[index + 1] = temporary;
            }
            index += 1;
        }
        end -= 1;
    }
}

pub fn selection_sort(values: &mut Vec<i64>) {
    let mut start = 0;
    while start < values.len() {
        let mut smallest = start;
        let mut index = start + 1;
        while index < values.len() {
            if values[index] < values[smallest] {
                smallest = index;
            }
            index += 1;
        }
        if smallest != start {
            let temporary = values[start];
            values[start] = values[smallest];
            values[smallest] = temporary;
        }
        start += 1;
    }
}

pub fn insertion_sort(values: &mut Vec<i64>) {
    let mut index = 1;
    while index < values.len() {
        let current = values[index];
        let mut position = index;
        while position > 0 && values[position - 1] > current {
            values[position] = values[position - 1];
            position -= 1;
        }
        values[position] = current;
        index += 1;
    }
}

pub fn sorted_copy(values: &Vec<i64>) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut index = 0;
    while index < values.len() {
        result.push(values[index]);
        index += 1;
    }
    insertion_sort(&mut result);
    result
}

pub fn merge_sorted(left: &Vec<i64>, right: &Vec<i64>) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut first = 0;
    let mut second = 0;
    while first < left.len() && second < right.len() {
        if left[first] <= right[second] {
            result.push(left[first]);
            first += 1;
        } else {
            result.push(right[second]);
            second += 1;
        }
    }
    while first < left.len() {
        result.push(left[first]);
        first += 1;
    }
    while second < right.len() {
        result.push(right[second]);
        second += 1;
    }
    result
}

pub fn range_vec(start: i64, end: i64) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut value = start;
    while value < end {
        result.push(value);
        value += 1;
    }
    result
}

pub fn squares_upto(limit: i64) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut value = 1;
    while value <= limit {
        result.push(value * value);
        value += 1;
    }
    result
}

pub fn cumulative_sums(values: &Vec<i64>) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut running = 0;
    let mut index = 0;
    while index < values.len() {
        running += values[index];
        result.push(running);
        index += 1;
    }
    result
}

pub fn deltas(values: &Vec<i64>) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    let mut index = 1;
    while index < values.len() {
        result.push(values[index] - values[index - 1]);
        index += 1;
    }
    result
}

pub fn running_max(values: &Vec<i64>) -> Vec<i64> {
    let mut result: Vec<i64> = Vec::new();
    if values.len() == 0 {
        return result;
    }
    let mut best = values[0];
    result.push(best);
    let mut index = 1;
    while index < values.len() {
        if values[index] > best {
            best = values[index];
        }
        result.push(best);
        index += 1;
    }
    result
}

pub fn dot_product(left: &Vec<i64>, right: &Vec<i64>) -> i64 {
    let limit = if left.len() < right.len() {
        left.len()
    } else {
        right.len()
    };
    let mut total = 0;
    let mut index = 0;
    while index < limit {
        total += left[index] * right[index];
        index += 1;
    }
    total
}

pub fn squared_distance(left: &Vec<i64>, right: &Vec<i64>) -> i64 {
    let limit = if left.len() < right.len() {
        left.len()
    } else {
        right.len()
    };
    let mut total = 0;
    let mut index = 0;
    while index < limit {
        let difference = left[index] - right[index];
        total += difference * difference;
        index += 1;
    }
    total
}

// sorted_checksum: sort a copy ascending, then fold value * (position + 1).
pub fn sorted_checksum(values: &Vec<i64>) -> i64 {
    let sorted = sorted_copy(values);
    let mut total = 0;
    let mut index = 0;
    while index < sorted.len() {
        total += sorted[index] * ((index as i64) + 1);
        index += 1;
    }
    total % MODULUS
}

// IntList: an owned vector wrapper with aggregate queries.
pub struct IntList {
    values: Vec<i64>,
}

impl IntList {
    pub fn new() -> IntList {
        IntList {
            values: Vec::new(),
        }
    }

    pub fn from_values(values: Vec<i64>) -> IntList {
        IntList { values: values }
    }

    pub fn push(&mut self, value: i64) {
        self.values.push(value);
    }

    pub fn pop(&mut self) -> Option<i64> {
        self.values.pop()
    }

    pub fn len(&self) -> i64 {
        self.values.len() as i64
    }

    pub fn total(&self) -> i64 {
        sum_vec(&self.values)
    }

    pub fn maximum(&self) -> i64 {
        max_value(&self.values)
    }

    pub fn minimum(&self) -> i64 {
        min_value(&self.values)
    }

    pub fn average(&self) -> i64 {
        if self.values.len() == 0 {
            return 0;
        }
        self.total() / self.values.len() as i64
    }

    pub fn sorted(&self) -> Vec<i64> {
        sorted_copy(&self.values)
    }

    pub fn reverse(&mut self) {
        reverse_in_place(&mut self.values);
    }

    pub fn scale(&mut self, factor: i64) {
        let mut index = 0;
        while index < self.values.len() {
            self.values[index] = self.values[index] * factor;
            index += 1;
        }
    }

    pub fn even_values(&self) -> Vec<i64> {
        let mut result: Vec<i64> = Vec::new();
        let mut index = 0;
        while index < self.values.len() {
            if self.values[index] % 2 == 0 {
                result.push(self.values[index]);
            }
            index += 1;
        }
        result
    }

    pub fn contains(&self, wanted: i64) -> bool {
        first_index_of(&self.values, wanted) >= 0
    }

    pub fn append(&mut self, other: Vec<i64>) {
        let mut index = 0;
        while index < other.len() {
            self.values.push(other[index]);
            index += 1;
        }
    }

    pub fn checksum(&self, modulus: i64) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.values.len() {
            total = (total * 31 + self.values[index]) % modulus;
            index += 1;
        }
        total
    }

    pub fn summary(&self) -> String {
        format!(
            "n={} sum={} min={} max={} avg={}",
            self.len(),
            self.total(),
            self.minimum(),
            self.maximum(),
            self.average()
        )
    }
}

// Grid: a row-major integer matrix stored in one Vec.
pub struct Grid {
    rows: i64,
    cols: i64,
    cells: Vec<i64>,
}

impl Grid {
    pub fn new(rows: i64, cols: i64) -> Grid {
        let mut cells: Vec<i64> = Vec::with_capacity((rows * cols) as usize);
        let mut index = 0;
        while index < rows * cols {
            cells.push(0);
            index += 1;
        }
        Grid {
            rows: rows,
            cols: cols,
            cells: cells,
        }
    }

    pub fn rows(&self) -> i64 {
        self.rows
    }

    pub fn cols(&self) -> i64 {
        self.cols
    }

    pub fn get(&self, row: i64, col: i64) -> i64 {
        self.cells[(row * self.cols + col) as usize]
    }

    pub fn set(&mut self, row: i64, col: i64, value: i64) {
        self.cells[(row * self.cols + col) as usize] = value;
    }

    pub fn total(&self) -> i64 {
        sum_vec(&self.cells)
    }

    pub fn row_sum(&self, row: i64) -> i64 {
        let mut total = 0;
        let mut col = 0;
        while col < self.cols {
            total += self.get(row, col);
            col += 1;
        }
        total
    }

    pub fn column_sum(&self, col: i64) -> i64 {
        let mut total = 0;
        let mut row = 0;
        while row < self.rows {
            total += self.get(row, col);
            row += 1;
        }
        total
    }

    pub fn diagonal_sum(&self) -> i64 {
        let limit = if self.rows < self.cols {
            self.rows
        } else {
            self.cols
        };
        let mut total = 0;
        let mut index = 0;
        while index < limit {
            total += self.get(index, index);
            index += 1;
        }
        total
    }

    pub fn fill_row(&mut self, row: i64, value: i64) {
        let mut col = 0;
        while col < self.cols {
            self.set(row, col, value * (col + 1));
            col += 1;
        }
    }

    pub fn transpose_cells(&self) -> Vec<i64> {
        let mut result: Vec<i64> = Vec::with_capacity((self.rows * self.cols) as usize);
        let mut col = 0;
        while col < self.cols {
            let mut row = 0;
            while row < self.rows {
                result.push(self.get(row, col));
                row += 1;
            }
            col += 1;
        }
        result
    }

    pub fn checksum(&self) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.cells.len() {
            total = (total * 31 + self.cells[index]) % MODULUS;
            index += 1;
        }
        total
    }
}

// iterator_checksum: exercise the lazy iterator adapters on a Vec<i64>.
pub fn iterator_checksum(values: Vec<i64>) -> i64 {
    let total: i64 = values
        .iter()
        .filter(|value| *value % 2 == 0)
        .map(|value| value * 3)
        .fold(0, |accumulator, value| accumulator + value);
    total % MODULUS
}

// enumerate_checksum: fold (index + 1) * value over a slice iterator.
pub fn enumerate_checksum(values: Vec<i64>) -> i64 {
    let mut total = 0;
    for pair in values.iter().enumerate() {
        let (index, value) = pair;
        total += ((index as i64) + 1) * value;
    }
    total % MODULUS
}

// count_greater uses any/all/find on an iterator pipeline.
pub fn count_greater(values: Vec<i64>, limit: i64) -> i64 {
    let found = values.iter().find(|value| *value > limit);
    match found {
        Some(value) => value,
        None => 0,
    }
}

pub fn any_greater(values: Vec<i64>, limit: i64) -> bool {
    values.iter().any(|value| *value > limit)
}

pub fn all_positive(values: Vec<i64>) -> bool {
    values.iter().all(|value| *value > 0)
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;

    let small: Vec<i64> = vec![1, 2, 3, 4, 5];
    acc = (acc * 31 + sum_vec(&small)) % MODULUS;

    let mixed: Vec<i64> = vec![3, 91, 27, 44];
    acc = (acc * 31 + max_value(&mixed)) % MODULUS;

    let looked: Vec<i64> = vec![7, 3, 9, 1, 5];
    acc = (acc * 31 + sorted_checksum(&looked)) % MODULUS;

    let ascending: Vec<i64> = vec![1, 2, 4, 8, 16, 32];
    acc = (acc * 31 + binary_search(&ascending, 16)) % MODULUS;

    let first: Vec<i64> = vec![1, 2, 3];
    let second: Vec<i64> = vec![4, 5, 6];
    acc = (acc * 31 + dot_product(&first, &second)) % MODULUS;

    let spanned: Vec<i64> = range_vec(1, 101);
    acc = (acc * 31 + sum_vec(&spanned)) % MODULUS;

    let pipeline: Vec<i64> = vec![5, 12, 7, 20, 3, 8];
    acc = (acc * 31 + iterator_checksum(pipeline)) % MODULUS;

    acc % MODULUS
}
