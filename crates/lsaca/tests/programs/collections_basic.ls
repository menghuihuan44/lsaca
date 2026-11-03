// Collection containers: insertion ordered sets, sorted sets and maps,
// double ended queues and binary heaps.

fn main() {
    let mut seen: HashSet<i64> = HashSet::new();
    println!("{}", seen.insert(3));
    println!("{}", seen.insert(3));
    println!("{}", seen.insert(7));
    println!("{}", seen.insert(5));
    println!("{} {}", seen.len(), seen.contains(7));
    println!("{:?}", seen.elements());

    let mut other: HashSet<i64> = HashSet::new();
    other.insert(5);
    other.insert(9);
    println!("{:?}", seen.union(&other).elements());
    println!("{:?}", seen.intersection(&other).elements());
    println!("{:?}", seen.difference(&other).elements());
    println!("{} {}", seen.subset(&seen), other.subset(&seen));
    println!("{}", seen.remove(3));
    println!("{:?}", seen.elements());

    let mut ordered: BTreeSet<i64> = BTreeSet::new();
    ordered.insert(9);
    ordered.insert(2);
    ordered.insert(5);
    ordered.insert(5);
    println!("{:?}", ordered.elements());
    println!("{} {}", ordered.contains(5), ordered.first().unwrap());
    println!("{}", ordered.last().unwrap());
    println!("{}", ordered.remove(9));
    println!("{:?}", ordered.elements());

    let mut tree: BTreeMap<String, i64> = BTreeMap::new();
    tree.insert(String::from("b"), 2);
    tree.insert(String::from("a"), 1);
    tree.insert(String::from("b"), 20);
    println!("{:?} {:?}", tree.keys(), tree.values());
    println!("{:?}", tree.get(String::from("a")));
    println!("{}", tree.contains_key(String::from("b")));
    println!("{:?}", tree.remove(String::from("a")));
    println!("{}", tree.len());

    let mut queue: VecDeque<i64> = VecDeque::new();
    queue.push_back(2);
    queue.push_back(3);
    queue.push_front(1);
    println!("{:?} {}", queue.elements(), queue.len());
    println!("{:?} {:?}", queue.pop_front(), queue.pop_back());
    println!("{:?}", queue.get(0));

    let mut heap: BinaryHeap<i64> = BinaryHeap::new();
    heap.push(3);
    heap.push(9);
    heap.push(5);
    println!("{:?} {:?}", heap.elements(), heap.peek());
    println!("{:?} {:?}", heap.pop(), heap.pop());
    println!("{}", heap.len());
}
