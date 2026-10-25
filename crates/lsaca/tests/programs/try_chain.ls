// Two try operators in one function plus try inside method bodies.
struct Store {
    items: Vec<i64>,
}

impl Store {
    fn first(&self) -> Option<i64> {
        let value = self.items.first()?;
        Some(value)
    }

    fn second(&self) -> Option<i64> {
        let first = self.first()?;
        let second = self.items.get(1)?;
        Some(first * 10 + second)
    }
}

fn total(store: Store) -> Option<i64> {
    let first = store.first()?;
    let second = store.second()?;
    Some(first + second)
}

fn main() {
    let store = Store {
        items: vec![2, 5],
    };
    match store.second() {
        Some(value) => println!("second {}", value),
        None => println!("none"),
    }
    match total(store) {
        Some(value) => println!("total {}", value),
        None => println!("none"),
    }
}