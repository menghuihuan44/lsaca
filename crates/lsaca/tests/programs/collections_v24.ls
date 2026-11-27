// v2.4 collection API density. get_or_insert and get_or_insert_with always
// re-look-up the key after insertion because growing the map can rehash it;
// a borrowed Entry is deferred to v2.6 (RFC 0002).

fn main() {
    let mut scores: HashMap<String, i64> = HashMap::new();
    let first = scores.get_or_insert(String::from("a"), 10);
    println!("first {}", *first);
    *first = 11;
    let again = scores.get_or_insert(String::from("a"), 99);
    println!("again {}", *again);

    let inserted = scores.get_or_insert_with(String::from("b"), || {
        println!("computed b");
        20
    });
    println!("inserted {}", *inserted);

    let existing = scores.get_or_insert_with(String::from("b"), || {
        println!("must not run");
        99
    });
    println!("existing {}", *existing);

    let seed = 30;
    let captured = scores.get_or_insert_with(String::from("c"), || {
        println!("computed c");
        seed + 1
    });
    println!("captured {}", *captured);

    // Grow the table first, so the lookup after get_or_insert has to survive
    // a rehash rather than reusing a reference taken before the insert.
    let mut grown: HashMap<i64, i64> = HashMap::new();
    let mut index: i64 = 0;
    while index < 20 {
        grown.insert(index, index * 2);
        index = index + 1;
    }
    let fresh = grown.get_or_insert(100, 7);
    println!("grown {} {}", *fresh, grown.len());

    let mut set: HashSet<i64> = HashSet::new();
    set.insert(2);
    set.insert(1);
    set.extend(vec![4i64, 3]);
    println!("set {:?} {}", set.to_vec(), set.len());

    let btree: BTreeSet<i64> = BTreeSet::from(vec![3i64, 1, 2, 1]);
    println!("btree {:?} {:?} {:?}", btree.to_vec(), btree.first(), btree.last());

    let mut bmap: BTreeMap<String, i64> = BTreeMap::from(vec![
        (String::from("b"), 2i64),
        (String::from("a"), 1i64),
    ]);
    println!("bmap {:?}", bmap.to_vec());
    println!("first key {:?} last key {:?}", bmap.first_key(), bmap.last_key());
    match bmap.get_mut(String::from("a")) {
        Some(value) => { *value = 10; }
        None => {}
    }
    println!("bmap a {:?}", bmap.get(String::from("a")));

    let mut deque: VecDeque<i64> = VecDeque::with_capacity(4);
    deque.push_back(2);
    deque.push_back(3);
    deque.push_front(1);
    println!("deque {:?} {:?} {:?}", deque.to_vec(), deque.front(), deque.back());
    match deque.get_mut(1) {
        Some(value) => { *value = 20; }
        None => {}
    }
    println!("deque get {:?} {:?}", deque.get(1), deque.to_vec());
    let from_vec: VecDeque<i64> = VecDeque::from(vec![7i64, 8]);
    println!("deque from {:?}", from_vec.to_vec());

    let heap: BinaryHeap<i64> = BinaryHeap::from(vec![3i64, 1, 2]);
    println!("heap {:?}", heap.to_vec());
}
