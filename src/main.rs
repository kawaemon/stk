mod hex;

use std::{fs::File, io::BufReader};

use crate::hex::decode_intel_hex;

fn main() {
    let decoded = decode_intel_hex(BufReader::new(File::open("./out.hex").unwrap())).unwrap();

    for (i, &b) in decoded.iter().enumerate() {
        if i % 32 == 0 {
            println!();
            print!("{i:05x}: ");
        }
        print!("{b:02x} ");
    }
    println!();

    assert_eq!(decoded.len() % 2, 0);

    for instruction in decoded.chunks(2) {
        let &[a, b] = instruction else {
            unreachable!()
        };

        let instruction = ((b as u16) << 8) | (a as u16);
        println!("{instruction:016b}");

        let masked = instruction & 0b0011_1111_1001_1111;
        let is_noop = masked == 0;
        if is_noop {
            println!("noop")
        }

        let masked = instruction & 0b0011_1000_0000_0000;
        let is_goto = (masked ^ 0b0010_1000_0000_0000) == 0;
        if is_goto {
            let k = instruction & 0b0000_0111_1111_1111;
            println!("goto: k = {k:016b}");
        }
    }
}
