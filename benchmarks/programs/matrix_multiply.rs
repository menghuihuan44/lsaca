// matrix_multiply: two deterministic 160x160 i64 matrices in flat arrays
// (index i*n+j), naive i-j-k triple loop into a third flat array, then the sum
// of every result element.
const N: i64 = 160;

fn value_a(i: i64, j: i64) -> i64 {
    (i * 31 + j * 17) % 1000
}

fn value_b(i: i64, j: i64) -> i64 {
    (i * 13 + j * 7) % 1000
}

fn main() {
    let cells = (N * N) as usize;
    let mut a = vec![0i64; cells];
    let mut b = vec![0i64; cells];
    let mut c = vec![0i64; cells];

    for i in 0..N {
        for j in 0..N {
            a[(i * N + j) as usize] = value_a(i, j);
            b[(i * N + j) as usize] = value_b(i, j);
        }
    }

    for i in 0..N {
        for j in 0..N {
            let mut sum = 0i64;
            for k in 0..N {
                sum += a[(i * N + k) as usize] * b[(k * N + j) as usize];
            }
            c[(i * N + j) as usize] = sum;
        }
    }

    let total: i64 = c.iter().sum();
    println!("checksum={}", total);
}
