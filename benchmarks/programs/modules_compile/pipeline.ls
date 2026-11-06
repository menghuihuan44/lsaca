// pipeline: lazy iterator pipelines over owned vectors.
// Part of the modules_compile compile-time workload.
//
// Every function exercises a different adapter or terminal operation:
// filter, map, fold, collect, skip, take, enumerate, any, all, find, count,
// sum and for-loops over adapter chains. The inputs are built with index
// loops, so the pipelines themselves stay one-liner style.

pub const MODULUS: i64 = 1000000007;

pub fn numbers(limit: i64) -> Vec<i64> {
    let mut values: Vec<i64> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(index + 1);
        index += 1;
    }
    values
}

pub fn squares(limit: i64) -> Vec<i64> {
    let mut values: Vec<i64> = Vec::with_capacity(limit as usize);
    let mut index = 1;
    while index <= limit {
        values.push(index * index);
        index += 1;
    }
    values
}

pub fn words() -> Vec<String> {
    let mut values: Vec<String> = Vec::with_capacity(8);
    values.push(String::from("alpha"));
    values.push(String::from("beta"));
    values.push(String::from("gamma"));
    values.push(String::from("delta"));
    values.push(String::from("epsilon"));
    values.push(String::from("zeta"));
    values
}

pub fn sum_even_squares(limit: i64) -> i64 {
    let values = numbers(limit);
    values.iter().filter(|n| *n % 2 == 0).map(|n| n * n).fold(0, |acc, item| acc + item)
}

pub fn product_odd(limit: i64) -> i64 {
    let values = numbers(limit);
    let odds: Vec<i64> = values.iter().filter(|n| *n % 2 == 1).collect();
    let mut product = 1;
    let mut index = 0;
    while index < odds.len() {
        product = product * odds[index];
        index += 1;
    }
    product
}

pub fn count_above(limit: i64, threshold: i64) -> i64 {
    let values = numbers(limit);
    values.iter().filter(|n| *n > threshold).count() as i64
}

pub fn sum_skip_take(limit: i64) -> i64 {
    let values = numbers(limit);
    values.iter().skip(2).take(4).sum()
}

pub fn collect_evens(limit: i64) -> Vec<i64> {
    let values = numbers(limit);
    let kept: Vec<i64> = values.iter().filter(|n| *n % 2 == 0).collect();
    kept
}

pub fn collect_doubles(limit: i64) -> Vec<i64> {
    let values = numbers(limit);
    let doubled: Vec<i64> = values.iter().map(|n| n * 2).collect();
    doubled
}

pub fn collect_cubes(limit: i64) -> Vec<i64> {
    let values = numbers(limit);
    let cubes = values.iter().map(|n| n * n).map(|n| n * 3).collect::<Vec<i64>>();
    cubes
}

pub fn enumerate_sum(limit: i64) -> i64 {
    let values = numbers(limit);
    let mut total = 0;
    for pair in values.iter().enumerate() {
        let (index, value) = pair;
        total += (index as i64) * value;
    }
    total as i64
}

pub fn chain_sum(limit: i64) -> i64 {
    let values = numbers(limit);
    values.iter().filter(|n| *n % 2 == 0).map(|n| n * n).fold(0, |acc, item| acc + item)
}

pub fn chain_product(limit: i64) -> i64 {
    let values = numbers(limit);
    values.iter().filter(|n| *n % 3 == 0).map(|n| n / 3).fold(1, |acc, item| acc * item)
}

pub fn nested_chain(limit: i64) -> i64 {
    let values = squares(limit);
    values.iter().filter(|n| *n % 3 == 0).map(|n| n / 3).filter(|n| *n > 2).fold(0, |acc, item| acc + item)
}

pub fn joined_words(limit: i64) -> String {
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("w{}", index % 7));
        index += 1;
    }
    let joined = values.iter().fold(String::from(""), |acc, item| acc + &item);
    joined
}

pub fn shouted_words(limit: i64) -> Vec<String> {
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("w{}", index));
        index += 1;
    }
    let shouted = values.iter().map(|word| word + "!").collect::<Vec<String>>();
    shouted
}

pub fn word_lengths(limit: i64) -> Vec<i64> {
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("value-{}", index));
        index += 1;
    }
    let lengths: Vec<i64> = values.iter().map(|word| word.len() as i64).collect();
    lengths
}

pub fn longest_word_length(limit: i64) -> i64 {
    let base = words();
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("{}-{}", base[0], index));
        index += 1;
    }
    let lengths: Vec<i64> = values.iter().map(|word| word.len() as i64).collect();
    let mut longest = 0;
    let mut index = 0;
    while index < lengths.len() {
        if lengths[index] > longest {
            longest = lengths[index];
        }
        index += 1;
    }
    longest
}

pub fn total_word_length(limit: i64) -> i64 {
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("word{}", index));
        index += 1;
    }
    values.iter().map(|word| word.len() as i64).sum()
}

pub fn any_longer_than(limit: i64, threshold: i64) -> bool {
    let base = words();
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("{}-{}", base[1], index));
        index += 1;
    }
    values.iter().any(|word| word.len() > (threshold as usize))
}

pub fn all_shorter_than(limit: i64, threshold: i64) -> bool {
    let base = words();
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("{}-{}", base[2], index));
        index += 1;
    }
    values.iter().all(|word| word.len() < (threshold as usize))
}

pub fn find_at_least(values: Vec<i64>, threshold: i64) -> i64 {
    let found = values.iter().find(|n| *n > threshold);
    match found {
        Some(value) => value,
        None => -1,
    }
}

pub fn first_above_index(limit: i64, threshold: i64) -> i64 {
    let values = squares(limit);
    let found = values.iter().find(|n| *n > threshold);
    match found {
        Some(value) => value / threshold,
        None => 0,
    }
}

pub fn row_lengths(matrix: Vec<Vec<i64>>) -> Vec<i64> {
    let lengths: Vec<i64> = matrix.iter().map(|row| row.len() as i64).collect();
    lengths
}

pub fn row_totals(matrix: &Vec<Vec<i64>>) -> Vec<i64> {
    let mut totals: Vec<i64> = Vec::with_capacity(matrix.len());
    let mut row = 0;
    while row < matrix.len() {
        let row_values = matrix[row].clone();
        let total: i64 = row_values.iter().sum();
        totals.push(total);
        row += 1;
    }
    totals
}

pub fn matrix_flat_total(matrix: &Vec<Vec<i64>>) -> i64 {
    let mut total = 0;
    let mut row = 0;
    while row < matrix.len() {
        let row_values = matrix[row].clone();
        total += row_values.iter().fold(0, |acc, item| acc + item);
        row += 1;
    }
    total
}

pub fn lazy_locals(limit: i64) -> i64 {
    let values = numbers(limit);
    let doubled = values.iter().map(|n| n * 2);
    let kept = doubled.filter(|n| *n > 4);
    let mut total = 0;
    for value in kept {
        total += value;
    }
    total
}

pub fn captured_offset(limit: i64, offset: i64) -> i64 {
    let values = numbers(limit);
    let total: i64 = values.iter().fold(0, |acc, item| acc + item * offset);
    total
}

pub fn captured_suffix(limit: i64) -> Vec<String> {
    let suffix = String::from("!");
    let mut values: Vec<String> = Vec::with_capacity(limit as usize);
    let mut index = 0;
    while index < limit {
        values.push(format!("tag{}", index));
        index += 1;
    }
    let tagged: Vec<String> = values.iter().map(|word| format!("{}{}", word, suffix)).collect();
    tagged
}

pub fn window_sums(limit: i64) -> i64 {
    let values = numbers(limit);
    let mut total = 0;
    for value in values.iter().skip(1).take(5) {
        total = total + value;
        total = total + value;
    }
    total
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;

    acc = (acc * 31 + sum_even_squares(10)) % MODULUS;
    acc = (acc * 31 + product_odd(7)) % MODULUS;
    acc = (acc * 31 + count_above(20, 12)) % MODULUS;
    acc = (acc * 31 + sum_skip_take(10)) % MODULUS;

    let evens = collect_evens(10);
    acc = (acc * 31 + evens.len() as i64) % MODULUS;
    acc = (acc * 31 + evens.iter().sum()) % MODULUS;

    let doubles = collect_doubles(6);
    acc = (acc * 31 + doubles.len() as i64) % MODULUS;
    acc = (acc * 31 + doubles.iter().fold(0, |acc2, item| acc2 + item)) % MODULUS;

    let cubes = collect_cubes(4);
    acc = (acc * 31 + cubes.len() as i64) % MODULUS;
    acc = (acc * 31 + cubes[2]) % MODULUS;

    acc = (acc * 31 + enumerate_sum(6)) % MODULUS;
    acc = (acc * 31 + chain_sum(20)) % MODULUS;
    acc = (acc * 31 + chain_product(9)) % MODULUS;
    acc = (acc * 31 + nested_chain(8)) % MODULUS;

    let joined = joined_words(5);
    acc = (acc * 31 + joined.len() as i64) % MODULUS;

    let shouted = shouted_words(4);
    acc = (acc * 31 + shouted.len() as i64) % MODULUS;
    acc = (acc * 31 + shouted[1].len() as i64) % MODULUS;

    let lengths = word_lengths(5);
    acc = (acc * 31 + lengths.len() as i64) % MODULUS;
    acc = (acc * 31 + lengths.iter().sum()) % MODULUS;

    acc = (acc * 31 + longest_word_length(3)) % MODULUS;
    acc = (acc * 31 + total_word_length(4)) % MODULUS;

    let has_long = any_longer_than(3, 6);
    if has_long {
        acc = (acc * 31 + 1) % MODULUS;
    }
    let all_short = all_shorter_than(3, 20);
    if all_short {
        acc = (acc * 31 + 1) % MODULUS;
    }

    let search: Vec<i64> = vec![4, 8, 15, 16, 23, 42];
    acc = (acc * 31 + find_at_least(search, 15)) % MODULUS;
    acc = (acc * 31 + first_above_index(9, 20)) % MODULUS;

    let matrix: Vec<Vec<i64>> = vec![
        vec![1, 2, 3],
        vec![4, 5],
        vec![6, 7, 8, 9],
    ];
    let lengths = row_lengths(matrix);
    acc = (acc * 31 + lengths.iter().sum()) % MODULUS;

    let grid: Vec<Vec<i64>> = vec![
        vec![2, 4, 6],
        vec![1, 3, 5],
        vec![7, 8],
    ];
    let totals = row_totals(&grid);
    acc = (acc * 31 + totals.len() as i64) % MODULUS;
    acc = (acc * 31 + totals[2]) % MODULUS;
    acc = (acc * 31 + matrix_flat_total(&grid)) % MODULUS;

    acc = (acc * 31 + lazy_locals(8)) % MODULUS;
    acc = (acc * 31 + captured_offset(5, 3)) % MODULUS;

    let tagged = captured_suffix(3);
    acc = (acc * 31 + tagged[2].len() as i64) % MODULUS;

    acc = (acc * 31 + window_sums(12)) % MODULUS;

    acc % MODULUS
}