use std::net::{SocketAddr, UdpSocket};

fn main() {
    let parsed = SocketAddr::parse("127.0.0.1:8080");
    match parsed {
        Ok(address) => {
            println!("ip {}", address.ip());
            println!("port {}", address.port());
            println!("text {}", address.to_string());
            println!("display {}", address.display());
        }
        Err(error) => {
            println!("parse error {}", error.message());
        }
    }
    let socket = UdpSocket::bind("127.0.0.1:0");
    match socket {
        Ok(mut socket) => {
            let local_port = socket.local_port();
            println!("local positive {}", local_port > 0);
            let target = format!("127.0.0.1:{}", local_port);
            let payload = String::from_str("ping");
            let sent = socket.send_to(payload.as_bytes(), target.as_str());
            match sent {
                Ok(count) => {
                    println!("sent {}", count);
                    let received = socket.recv_from(64);
                    match received {
                        Ok(message) => {
                            match message {
                                Some(pair) => {
                                    let (bytes, sender) = pair;
                                    println!("got {}", bytes.len());
                                    let mut text: String = String::new();
                                    let mut index: usize = 0;
                                    while index < bytes.len() {
                                        text.push(bytes.get(index).unwrap() as char);
                                        index += 1;
                                    }
                                    println!("payload {}", text);
                                    let peer = SocketAddr::parse(sender.as_str());
                                    match peer {
                                        Ok(peer) => {
                                            println!("sender ip {}", peer.ip() == String::from_str("127.0.0.1"));
                                            println!("sender port {}", peer.port() == local_port);
                                        }
                                        Err(error) => {
                                            println!("sender error {}", error.message());
                                        }
                                    }
                                }
                                None => {
                                    println!("no message");
                                }
                            }
                        }
                        Err(error) => {
                            println!("receive error {}", error.message());
                        }
                    }
                }
                Err(error) => {
                    println!("send error {}", error.message());
                }
            }
            let closed = socket.close();
            match closed {
                Ok(()) => {
                    println!("closed true");
                }
                Err(error) => {
                    println!("close error {}", error.message());
                }
            }
        }
        Err(error) => {
            println!("bind error {}", error.message());
        }
    }
}
