// generic_stack: the same generic stack instantiated for two element types, i64
// and String. Push one million integers, pop them all into a sum; push one
// thousand small strings, pop them all into a total byte length.
struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    fn new() -> Stack<T> {
        Stack { items: Vec::new() }
    }

    fn push(&mut self, item: T) {
        self.items.push(item);
    }

    fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    fn len(&self) -> i64 {
        self.items.len()
    }
}

fn main() {
    let n: i64 = 1000000;
    let mut numbers: Stack<i64> = Stack::new();
    let mut i: i64 = 0;
    while i < n {
        numbers.push(i);
        i += 1;
    }
    let mut total: i64 = 0;
    while numbers.len() > 0 {
        total += numbers.pop().unwrap();
    }

    let mut words: Stack<String> = Stack::new();
    let mut j: i64 = 0;
    while j < 1000 {
        words.push(format!("item{}", j % 100));
        j += 1;
    }
    let mut char_total: i64 = 0;
    while words.len() > 0 {
        char_total += words.pop().unwrap().len();
    }
    println!("checksum={},{}", total, char_total);
}
