// Property check: insertion sort preserves the length, the sum, the minimum
// and the maximum of the input and leaves a nondecreasing sequence.
fn next_value(seed: i64) -> i64 {
    (seed * 1103515245 + 12345) % 2147483648
}

fn insertion_sort(values: Vec<i64>) -> Vec<i64> {
    let mut data = values;
    let count = data.len();
    let mut index: i64 = 1;
    while index < count {
        let current = data[index];
        let mut position = index;
        while position > 0 {
            let previous = data[position - 1];
            if previous > current {
                data[position] = previous;
                position -= 1;
            } else {
                break;
            }
        }
        data[position] = current;
        index += 1;
    }
    data
}

fn main() {
    let count: i64 = 2000;
    let mut values: Vec<i64> = Vec::new();
    let mut seed: i64 = 12345;
    let mut index: i64 = 0;
    let mut sum_before: i64 = 0;
    let mut min_before: i64 = 0;
    let mut max_before: i64 = 0;
    while index < count {
        seed = next_value(seed);
        let value = seed % 2000001 - 1000000;
        values.push(value);
        sum_before += value;
        if index == 0 {
            min_before = value;
            max_before = value;
        } else {
            if value < min_before {
                min_before = value;
            }
            if value > max_before {
                max_before = value;
            }
        }
        index += 1;
    }
    let sorted = insertion_sort(values);
    assert_eq!(sorted.len(), count);
    let mut sum_after: i64 = 0;
    let mut ordered = true;
    let mut position: i64 = 0;
    while position < count {
        let value = sorted[position];
        sum_after += value;
        if position > 0 {
            let previous = sorted[position - 1];
            if previous > value {
                ordered = false;
            }
        }
        position += 1;
    }
    assert_eq!(sum_after, sum_before);
    assert_eq!(sorted[0], min_before);
    assert_eq!(sorted[count - 1], max_before);
    assert!(ordered);
    println!("ok sort");
}
