struct Inner {
    tags: Vec<String>,
    count: i64,
}

struct Outer {
    label: String,
    inner: Inner,
    scores: Vec<i64>,
}

fn push_tag(inner: &mut Inner, tag: String) {
    inner.tags.push(tag);
    inner.count = inner.count + 1;
}

fn record_score(outer: &mut Outer, score: i64) {
    outer.scores.push(score);
    outer.inner.count = outer.inner.count + 1;
}

fn relabel(outer: &mut Outer) {
    let existing: String = outer.label.clone();
    outer.label = String::from("outer:");
    outer.label.push_str(&existing);
}

fn main() {
    let mut outer = Outer {
        label: String::from("alpha"),
        inner: Inner {
            tags: Vec::new(),
            count: 0,
        },
        scores: Vec::new(),
    };
    push_tag(&mut outer.inner, String::from("first"));
    push_tag(&mut outer.inner, String::from("second"));
    record_score(&mut outer, 10);
    record_score(&mut outer, 25);
    relabel(&mut outer);
    println!("label {}", outer.label);
    println!("tags {:?} count {}", outer.inner.tags, outer.inner.count);
    println!("scores {:?} len {}", outer.scores, outer.scores.len());
    let mut inner_ref: &mut Inner = &mut outer.inner;
    inner_ref.tags.push(String::from("third"));
    inner_ref.count = inner_ref.count + 100;
    println!("after ref tags {} count {}", outer.inner.tags.len(), outer.inner.count);
    outer.inner.tags.reverse();
    println!("reversed tags {:?}", outer.inner.tags);
}
