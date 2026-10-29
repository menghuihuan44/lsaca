// fn_pipeline: a rule record carries a non-copy label string and a function
// value in the same struct. run() borrows the rule and calls its function value
// 1,000 times; the checksum sweeps three rules plus a copy of the first one, and
// reports the total label byte length.
struct Rule {
    label: String,
    apply: fn(i64) -> i64,
}

fn run(rule: &Rule, seed: i64, times: i64) -> i64 {
    let mut value = seed;
    let mut i = 0;
    while i < times {
        value = (rule.apply)(value);
        i += 1;
    }
    value
}

fn main() {
    let mut rules: Vec<Rule> = Vec::new();
    rules.push(Rule { label: String::from("add3"), apply: |x: i64| x + 3 });
    rules.push(Rule { label: String::from("modstep"), apply: |x: i64| (x * 3 + 1) % 1000003 });
    rules.push(Rule {
        label: String::from("collatz"),
        apply: |x: i64| if x % 2 == 0 { x / 2 } else { x * 3 + 1 },
    });

    let copy = rules[0].clone();
    let mut checksum: i64 = 0;
    let mut label_bytes: i64 = 0;
    checksum += run(&copy, 11, 500);
    label_bytes += copy.label.len();
    for rule in rules {
        checksum += run(&rule, 7, 1000);
        label_bytes += rule.label.len();
    }
    println!("checksum={},{}", checksum, label_bytes);
}
