use std::io::{self, Read};

/// decoder for <https://ja.wikipedia.org/wiki/Intel_HEX>
pub struct IntelHexDecoder<R> {
    reader: R,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error")]
    Io(#[from] io::Error),

    #[error("expected upper-case hex char(one of '0123456789ABCDEF'), found '{found}'")]
    InvalidHexChar { found: char },

    #[error("expected ':', found '{found}")]
    InvalidLineStart { found: char },

    #[error("unknown record type: {found}")]
    UnknownRecordType { found: u8 },

    #[error("expected '\\r\\n' or '\\n', found {found:?}")]
    InvalidNewLine { found: char },
}

type Result<T, E = Error> = std::result::Result<T, E>;

impl<R: Read> IntelHexDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    fn decode_hex_char(&mut self) -> Result<u8> {
        let mut buf = [0; 1];
        self.reader.read_exact(&mut buf).map_err(Error::Io)?;

        b"0123456789ABCDEF"
            .iter()
            .position(|&x| x == buf[0])
            .map(|p| p as u8)
            .ok_or(Error::InvalidHexChar { found: buf[0] as char })
    }

    fn decode_hex_u8(&mut self) -> Result<u8> {
        let c0 = self.decode_hex_char()?;
        let c1 = self.decode_hex_char()?;
        Ok(c0 << 4 | c1) // Big-Endian
    }

    fn decode_hex_u16(&mut self) -> Result<u16> {
        let c0 = self.decode_hex_u8()? as u16;
        let c1 = self.decode_hex_u8()? as u16;
        Ok(c0 << 8 | c1) // Big-Endian
    }

    pub fn decode(mut self) -> Result<Vec<u8>> {
        let mut decoded = vec![];

        let mut upper_address = 0u16;

        loop {
            let mut buf = [0; 1];
            self.reader.read_exact(&mut buf).map_err(Error::Io)?;

            if buf != [b':'] {
                return Err(Error::InvalidLineStart { found: buf[0] as char });
            }

            let byte_count = self.decode_hex_u8()?;
            let address = ((upper_address as u32) << 16) | self.decode_hex_u16()? as u32;

            let record_type = self.decode_hex_u8()?;

            match record_type {
                // data record
                0 => {
                    tracing::debug!("addr=0x{address:x}, bytes={byte_count}");
                    for i in 0..byte_count {
                        let b = self.decode_hex_u8()?;
                        let pos = (address + i as u32) as usize;
                        decoded.resize(pos + 1, 0);
                        decoded[pos] = b;
                    }
                }

                // EOF
                1 => break,

                4 => {
                    upper_address = self.decode_hex_u16()?;
                }

                i @ 2..=5 => unimplemented!("record type {i}"),

                _ => return Err(Error::UnknownRecordType { found: record_type }),
            }

            // FIXME: verify this
            let _checksum = self.decode_hex_u8();

            self.reader.read_exact(&mut buf).map_err(Error::Io)?;
            if buf == [b'\r'] {
                self.reader.read_exact(&mut buf).map_err(Error::Io)?;
            }
            if buf != [b'\n'] {
                return Err(Error::InvalidNewLine { found: buf[0] as char });
            }
        }

        Ok(decoded)
    }
}

pub fn decode_intel_hex<R: Read>(r: R) -> Result<Vec<u8>> {
    IntelHexDecoder::new(r).decode()
}
