mod hex;

use std::{fs::File, io::BufReader};

use crate::hex::decode_hex;

fn main() {
    let decoded = decode_hex(BufReader::new(File::open("./out.hex").unwrap())).unwrap();

    for (i, &b) in decoded.iter().enumerate() {
        if i % 32 == 0 {
            println!();
            print!("{i:05x}: ");
        }
        print!("{b:02x} ");
    }
    println!();
}
