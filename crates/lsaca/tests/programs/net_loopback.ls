fn main() {
    let address: String = String::from("127.0.0.1:0");
    let bound = std::net::TcpListener::bind(&address);
    if bound.is_err() {
        println!("bind skipped");
        return;
    }
    let listener = bound.unwrap();
    let port = listener.local_port();
    println!("port positive {}", port > 0);
    let mut target: String = String::from("127.0.0.1:");
    let port_text: String = format!("{}", port);
    target.push_str(&port_text);
    let connected = std::net::TcpStream::connect(&target);
    if connected.is_err() {
        println!("connect failed");
        return;
    }
    let mut stream = connected.unwrap();
    println!("timeout ok {}", stream.set_timeout(2000).is_ok());
    let accepted = listener.accept();
    if accepted.is_err() {
        println!("accept failed");
        return;
    }
    let mut peer = accepted.unwrap();
    let payload: String = String::from("ping");
    let sent = stream.write(payload.as_str().as_bytes());
    println!("sent {}", sent.unwrap());
    let mut buffer: Vec<u8> = Vec::new();
    while buffer.len() < 4 {
        buffer.push(0);
    }
    let received = peer.read(buffer.as_mut_slice());
    println!("received {}", received.unwrap());
    let mut reply: Vec<u8> = Vec::new();
    reply.push(112);
    reply.push(111);
    reply.push(110);
    reply.push(103);
    println!("reply sent {}", peer.write(reply.as_slice()).unwrap());
    println!("shutdown ok {}", stream.shutdown(std::net::Shutdown::Both).is_ok());
}
