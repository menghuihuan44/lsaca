// matrix_multiply: two deterministic 160x160 i64 matrices in flat Vecs
// (index i*n+j), naive i-j-k triple loop into a third flat Vec, then the sum of
// every result element.
fn value_a(i: i64, j: i64) -> i64 {
    (i * 31 + j * 17) % 1000
}

fn value_b(i: i64, j: i64) -> i64 {
    (i * 13 + j * 7) % 1000
}

fn main() {
    let n: i64 = 160;
    let cells: i64 = n * n;
    let mut a = vec![0; cells];
    let mut b = vec![0; cells];
    let mut c = vec![0; cells];

    let mut i = 0;
    while i < n {
        let mut j = 0;
        while j < n {
            a[i * n + j] = value_a(i, j);
            b[i * n + j] = value_b(i, j);
            j += 1;
        }
        i += 1;
    }

    i = 0;
    while i < n {
        let mut j = 0;
        while j < n {
            let mut sum: i64 = 0;
            let mut k = 0;
            while k < n {
                sum += a[i * n + k] * b[k * n + j];
                k += 1;
            }
            c[i * n + j] = sum;
            j += 1;
        }
        i += 1;
    }

    let mut total: i64 = 0;
    for x in c {
        total += x;
    }
    println!("checksum={}", total);
}
