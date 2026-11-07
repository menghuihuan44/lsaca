fn main() {
    let mut count: i64 = 0;
    'outer: for i in 0..4 {
        for j in 0..4 {
            if j == 2 {
                continue 'outer;
            }
            if i == 3 {
                break 'outer;
            }
            count = count + 1;
        }
    }
    println!("{}", count);

    let mut n: i64 = 0;
    'timer: loop {
        n = n + 1;
        if n >= 3 {
            break 'timer;
        }
    }
    println!("{}", n);

    let mut total: i64 = 0;
    let mut index: i64 = 0;
    'scan: while index < 10 {
        index = index + 1;
        if index % 2 == 0 {
            continue 'scan;
        }
        if index > 7 {
            break 'scan;
        }
        total = total + index;
    }
    println!("{}", total);
}
