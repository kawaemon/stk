use std::{
    fs::File,
    io::{BufRead, BufReader},
};

fn main() {
    let mut hex_file = BufReader::new(File::open("./out.hex").unwrap());

    let mut decoded = vec![];

    for line in hex_file.lines() {
        let line = line.unwrap().chars().collect::<Vec<_>>();
        assert_eq!(line[0], ':');

        let byte_count = decode_hex_u8((line[1], line[2])).unwrap();
        let address = decode_hex_u16((line[3], line[4], line[5], line[6])).unwrap();

        let record_type = decode_hex_u8((line[7], line[8])).unwrap();

        let mut i = 9;
        let mut next = || {
            let v = i;
            i += 1;
            v
        };
        match record_type {
            // data record
            0 => {
                println!("addr=0x{address:x}, bytes={byte_count}");
                for i in 0..byte_count {
                    let b = decode_hex_u8((line[next() as usize], line[next() as usize])).unwrap();
                    if decoded.len() <= (address + i as u16) as usize {
                        let needs = (address + i as u16) as usize - decoded.len() + 1;
                        for _ in 0..needs {
                            decoded.push(0);
                        }
                    }
                    decoded[(address + i as u16) as usize] = b;
                }
            }
            // EOF
            1 => {
                break;
            }
            _ => unimplemented!(),
        }
    }

    for (i, &b) in decoded.iter().enumerate() {
        if i % 32 == 0 {
            println!();
            print!("{i:05x}: ");
        }
        print!("{b:02x} ");
    }
    println!();
}

fn decode_hex_char(c: char) -> Option<u8> {
    "0123456789ABCDEF"
        .chars()
        .position(|x| x == c)
        .map(|x| x as u8)
}

fn decode_hex_u8(c: (char, char)) -> Option<u8> {
    let c0 = decode_hex_char(c.0)?;
    let c1 = decode_hex_char(c.1)?;
    Some(c0 << 4 | c1) // Big-Endian
}

fn decode_hex_u16(c: (char, char, char, char)) -> Option<u16> {
    let c0 = decode_hex_u8((c.0, c.1))? as u16;
    let c1 = decode_hex_u8((c.2, c.3))? as u16;
    Some(c0 << 8 | c1) // Big-Endian
}
