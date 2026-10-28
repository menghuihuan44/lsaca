// Passing a value to a by-value parameter moves it into the callee.
struct Packet {
    label: String,
    values: Vec<i64>,
}

fn consume(packet: Packet) {
    println!("consume {} {:?}", packet.label, packet.values);
}

fn main() {
    let packet = Packet {
        label: String::from("first"),
        values: vec![1, 2, 3],
    };
    consume(packet);
    println!("done");
    let second = Packet {
        label: String::from("second"),
        values: vec![4],
    };
    consume(second);
}
