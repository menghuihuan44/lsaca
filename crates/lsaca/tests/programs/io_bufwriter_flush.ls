// BufWriter buffers writes and flushes them to a boxed Write trait object.
use std::io::{BufWriter, File, Write};

fn main() {
    let path: String = String::from("io_bufwriter_flush.txt");
    let created = File::create(&path);
    if created.is_err() {
        println!("create failed");
        return;
    }
    {
        let sink: Box<dyn Write> = Box::new(created.unwrap());
        let mut writer: BufWriter<Box<dyn Write>> = BufWriter::new(sink);
        let mut payload: Vec<u8> = Vec::new();
        payload.push(97);
        payload.push(108);
        payload.push(112);
        payload.push(104);
        payload.push(97);
        payload.push(45);
        payload.push(111);
        payload.push(109);
        payload.push(101);
        payload.push(103);
        payload.push(97);
        let count: usize = match writer.write(payload.as_slice()) {
            Ok(value) => {
                value
            }
            Err(error) => {
                println!("write failed");
                return;
            }
        };
        println!("buffered write {}", count);
        println!("flush ok {}", writer.flush().is_ok());
    }
    let opened = File::open(&path);
    if opened.is_err() {
        println!("open failed");
        return;
    }
    let mut file = opened.unwrap();
    match file.read_to_string() {
        Ok(text) => {
            println!("read {}", text);
        }
        Err(error) => {
            println!("read failed");
        }
    }
    println!("removed {}", std::fs::remove_file(&path).is_ok());
}
