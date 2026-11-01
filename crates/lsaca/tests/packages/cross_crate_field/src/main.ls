use pubcrate_lib::holder;

fn main() {
    let item = holder();
    println!("{}", item.value);
}
