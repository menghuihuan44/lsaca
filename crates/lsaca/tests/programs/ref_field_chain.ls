struct Inner {
    value: i64,
}

struct Outer {
    inner: Inner,
    other: i64,
}

fn bump(target: &mut i64) {
    *target = *target + 1;
}

fn main() {
    let mut outer = Outer { inner: Inner { value: 1 }, other: 2 };
    bump(&mut outer.inner.value);
    let shared = &outer.inner;
    let seen = (*shared).value;
    println!("{}", seen);

    let disjoint = &mut outer.other;
    *disjoint = *disjoint + seen;
    println!("{} {}", outer.inner.value, outer.other);
}
