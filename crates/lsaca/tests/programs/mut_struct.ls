// Mutable references to user structs with managed fields: the callee
// mutates a scalar field and a String field through `&mut Person`.

struct Person {
    name: String,
    age: i64,
}

fn birthday(person: &mut Person) {
    person.age = person.age + 1;
    person.name.push_str("!");
}

fn rename(person: &mut Person, name: String) {
    person.name = name;
}

fn main() {
    let mut person = Person { name: String::from("Ada"), age: 36 };
    birthday(&mut person);
    birthday(&mut person);
    println!("{} {}", person.name, person.age);
    rename(&mut person, String::from("Grace"));
    println!("{} {}", person.name, person.age);
}
