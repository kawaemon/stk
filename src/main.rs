use std::{fs::File, io::BufReader};

use stk::{hex::decode_intel_hex, inst::Instruction};

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    let decoded = decode_intel_hex(BufReader::new(File::open("./main.hex").unwrap())).unwrap();

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
        let decoded = Instruction::from_code(instruction);
        println!("{decoded:?}");
    }
}
