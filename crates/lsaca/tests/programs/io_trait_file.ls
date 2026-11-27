// File values implement Read and Write. The bytes travel through boxed trait
// objects so the fixture exercises dynamic dispatch, not the inherent methods.
use std::io::{File, Read, Write};

fn main() {
    let path: String = String::from("io_trait_file.txt");
    let created = File::create(&path);
    if created.is_err() {
        println!("create failed");
        return;
    }
    {
        let mut writer: Box<dyn Write> = Box::new(created.unwrap());
        let mut payload: Vec<u8> = Vec::new();
        payload.push(97);
        payload.push(108);
        payload.push(112);
        payload.push(104);
        payload.push(97);
        match writer.write_all(payload.as_slice()) {
            Ok(unit) => {
                println!("write_all ok");
            }
            Err(error) => {
                println!("write_all failed");
                return;
            }
        }
        match writer.flush() {
            Ok(unit) => {
                println!("flush ok");
            }
            Err(error) => {
                println!("flush failed");
                return;
            }
        }
    }
    let opened = File::open(&path);
    if opened.is_err() {
        println!("open failed");
        return;
    }
    {
        let mut reader: Box<dyn Read> = Box::new(opened.unwrap());
        let mut buffer: Vec<u8> = Vec::new();
        while buffer.len() < 8 {
            buffer.push(0);
        }
        let count: usize = match reader.read(buffer.as_mut_slice()) {
            Ok(value) => {
                value
            }
            Err(error) => {
                println!("read failed");
                return;
            }
        };
        println!("read {}", count);
        let mut bytes: Vec<u8> = Vec::new();
        let mut index: usize = 0;
        while index < count {
            bytes.push(buffer.get(index).unwrap());
            index = index + 1;
        }
        println!("bytes {:?}", bytes);
    }
    match std::fs::remove_file(&path) {
        Ok(unit) => {
            println!("removed");
        }
        Err(error) => {
            println!("remove failed");
        }
    }
}
