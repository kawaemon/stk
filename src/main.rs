use std::{
    fs::File,
    io::{BufRead, BufReader},
};

struct HexDecoder<R> {
    reader: R,
}

impl<R: BufRead> HexDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    fn decode_hex_char(&mut self) -> Option<u8> {
        let mut buf = [0; 1];
        self.reader.read_exact(&mut buf).ok()?;

        "0123456789ABCDEF"
            .chars()
            .position(|x| x as u8 == buf[0])
            .map(|p| p as u8)
    }

    fn decode_hex_u8(&mut self) -> Option<u8> {
        let c0 = self.decode_hex_char()?;
        let c1 = self.decode_hex_char()?;
        Some(c0 << 4 | c1) // Big-Endian
    }

    fn decode_hex_u16(&mut self) -> Option<u16> {
        let c0 = self.decode_hex_u8()? as u16;
        let c1 = self.decode_hex_u8()? as u16;
        Some(c0 << 8 | c1) // Big-Endian
    }

    pub fn decode(mut self) -> Option<Vec<u8>> {
        let mut decoded = vec![];

        loop {
            // TODO: decode newline, error handling
            let mut buf = [0; 1];
            self.reader.read_exact(&mut buf).unwrap();

            assert_eq!(buf[0], ':' as u8);

            let byte_count = self.decode_hex_u8()?;
            let address = self.decode_hex_u16()? as u32; // for extended linear address command

            let record_type = self.decode_hex_u8()?;

            match record_type {
                // data record
                0 => {
                    println!("addr=0x{address:x}, bytes={byte_count}");
                    for i in 0..byte_count {
                        let b = self.decode_hex_u8()?;
                        let pos = (address + i as u32) as usize;

                        if decoded.len() <= pos {
                            let needs = pos - decoded.len() + 1;
                            for _ in 0..needs {
                                decoded.push(0);
                            }
                        }
                        decoded[pos as usize] = b;
                    }
                }

                // EOF
                1 => break,

                2..=5 => unimplemented!(),

                _ => panic!("unknown record type"),
            }
        }

        Some(decoded)
    }
}

fn main() {
    let decoded = HexDecoder::new(BufReader::new(File::open("./out.hex").unwrap()))
        .decode()
        .unwrap();

    for (i, &b) in decoded.iter().enumerate() {
        if i % 32 == 0 {
            println!();
            print!("{i:05x}: ");
        }
        print!("{b:02x} ");
    }
    println!();
}
