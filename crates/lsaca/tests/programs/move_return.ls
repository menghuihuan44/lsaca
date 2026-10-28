// Return expressions and block tails move the value out of the function.
struct Packet {
    label: String,
    values: Vec<i64>,
}

fn make(label: String, values: Vec<i64>) -> Packet {
    Packet { label: label, values: values }
}

fn pass(packet: Packet) -> Packet {
    packet
}

fn echo(text: String) -> String {
    text
}

fn create() -> Packet {
    Packet { label: String::from("created"), values: Vec::new() }
}

fn main() {
    let packet = make(String::from("kept"), vec![1, 2]);
    let forwarded = pass(packet);
    println!("{} {:?}", forwarded.label, forwarded.values);

    let text = echo(String::from("lsaca"));
    println!("{}", text);

    let returned = create();
    println!("{}", returned.label);
}
