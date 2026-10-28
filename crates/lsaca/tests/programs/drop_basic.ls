// Drop runs at scope end in reverse declaration order; the user hook runs
// before the fields are released, and aggregates release their payloads.
struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

struct Pair {
    first: Loud,
    second: Loud,
}

impl Drop for Pair {
    fn drop(&mut self) {
        println!("drop pair");
    }
}

fn main() {
    println!("start");
    {
        let inner = Loud { name: String::from("inner") };
        println!("in block");
    }
    let outer = Loud { name: String::from("outer") };
    let pair = Pair {
        first: Loud { name: String::from("first") },
        second: Loud { name: String::from("second") },
    };
    let payloads = vec![
        Loud { name: String::from("v0") },
        Loud { name: String::from("v1") },
    ];
    let boxed = Box::new(Loud { name: String::from("boxed") });
    println!("end");
}
