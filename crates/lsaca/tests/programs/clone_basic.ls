// .clone() is the old v1.3 copy: String shares via retain, Vec and user
// structs are copied deeply, so mutating a clone leaves the source untouched.
struct Record {
    name: String,
    values: Vec<i64>,
}

fn main() {
    let text = String::from("lsaca");
    let copy = text.clone();
    println!("{} {}", text, copy);

    let mut numbers = vec![1, 2, 3];
    let numbers_copy = numbers.clone();
    numbers.push(4);
    println!("{:?} {:?}", numbers, numbers_copy);

    let record = Record {
        name: String::from("first"),
        values: vec![7, 8],
    };
    let mut record_copy = record.clone();
    record_copy.name = String::from("second");
    record_copy.values.push(9);
    println!("{} {:?}", record.name, record.values);
    println!("{} {:?}", record_copy.name, record_copy.values);
}
