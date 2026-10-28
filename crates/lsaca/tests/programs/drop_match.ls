struct Loud {
    name: String,
}

impl Drop for Loud {
    fn drop(&mut self) {
        println!("drop {}", self.name);
    }
}

fn make(name: String) -> Loud {
    Loud { name: name }
}

fn main() {
    println!("plain");
    let first = make(String::from("a"));
    match first {
        value => {
            println!("bound {}", value.name);
        }
    }

    println!("guard");
    let second = make(String::from("b"));
    match second {
        value if value.name == "b" => {
            println!("guard passed {}", value.name);
        }
        _ => {
            println!("unreachable");
        }
    }

    println!("guard_fail");
    let third = make(String::from("c"));
    match third {
        value if value.name == "zzz" => {
            println!("never {}", value.name);
        }
        other => {
            println!("fallback {}", other.name);
        }
    }

    println!("wildcard");
    let fourth = make(String::from("d"));
    match fourth {
        _ => {
            println!("ignored");
        }
    }

    println!("option");
    let wrapped = Some(make(String::from("e")));
    match wrapped {
        value => {
            println!("option bound {}", value.unwrap().name);
        }
    }

    println!("done");
}
