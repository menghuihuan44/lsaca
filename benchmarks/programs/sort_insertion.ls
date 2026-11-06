// sort_insertion: insertion sort over the same LCG sequence as sort_i64 - the
// alternative implementation for the sorting specification (same checksum
// rule: first, last, sum). Insertion sort is O(n^2), so the sizes are small.
// Optional argument: value count (default 1,000, the medium scale).
fn insertion_sort(values: Vec<i64>) -> Vec<i64> {
    let mut data = values;
    let n = data.len();
    let mut i: usize = 1;
    while i < n {
        let key = data[i];
        let mut j: usize = i;
        while j > 0 {
            let previous = j - 1;
            if data[previous] > key {
                data[j] = data[previous];
                j = previous;
            } else {
                break;
            }
        }
        data[j] = key;
        i = i + 1;
    }
    data
}

fn main() {
    let all = args();
    let n: i64 = if all.len() > 0 { all[0].clone().parse_i64().unwrap() } else { 1000 };
    let mut values: Vec<i64> = Vec::new();
    let mut state: i64 = 12345;
    let mut i: i64 = 0;
    while i < n {
        state = (16807 * state) % 2147483647;
        values.push(state);
        i = i + 1;
    }
    let sorted = insertion_sort(values);
    let mut sum: i64 = 0;
    let mut k: usize = 0;
    while k < sorted.len() {
        sum = sum + sorted[k];
        k = k + 1;
    }
    println!("checksum={},{},{}", sorted[0], sorted[(n - 1) as usize], sum);
}