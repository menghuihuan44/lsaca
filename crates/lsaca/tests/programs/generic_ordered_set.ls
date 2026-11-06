struct OrderedSet<K> {
    map: HashMap<K, ()>,
    order: Vec<K>,
}

impl<K> OrderedSet<K> {
    fn new() -> OrderedSet<K> {
        return OrderedSet { map: HashMap::new(), order: Vec::new() };
    }

    fn insert(&mut self, key: K) -> bool {
        if self.map.contains_key(key.clone()) {
            return false;
        }
        self.map.insert(key.clone(), ());
        self.order.push(key);
        return true;
    }

    fn contains(&self, key: K) -> bool {
        return self.map.contains_key(key);
    }

    fn remove(&mut self, key: K) -> bool {
        if !self.map.remove(key.clone()).is_some() {
            return false;
        }
        let mut remaining: Vec<K> = Vec::new();
        let mut index: usize = 0;
        while index < self.order.len() {
            let current: K = self.order.get(index).unwrap();
            if current != key.clone() {
                remaining.push(current);
            }
            index = index + 1;
        }
        self.order = remaining;
        return true;
    }

    fn len(&self) -> i64 {
        return self.order.len() as i64;
    }

    fn elements(&self) -> Vec<K> {
        return self.order.clone();
    }
}

fn main() {
    let mut words: OrderedSet<String> = OrderedSet::new();
    println!("insert {} {} {}", words.insert(String::from("beta")), words.insert(String::from("alpha")), words.insert(String::from("beta")));
    println!("elements {:?} len {}", words.elements(), words.len());
    println!("contains {} {}", words.contains(String::from("alpha")), words.contains(String::from("gamma")));
    println!("remove {} {}", words.remove(String::from("alpha")), words.remove(String::from("alpha")));
    println!("after remove {:?} len {}", words.elements(), words.len());
    let mut numbers: OrderedSet<i64> = OrderedSet::new();
    numbers.insert(4);
    numbers.insert(2);
    numbers.insert(4);
    numbers.insert(7);
    println!("numbers {:?} len {}", numbers.elements(), numbers.len());
    println!("numbers remove {} {:?}", numbers.remove(2), numbers.elements());
}
