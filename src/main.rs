#![allow(dead_code)]
mod hex;

use std::{fs::File, io::BufReader};

use crate::hex::decode_intel_hex;

fn main() {
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

        macro_rules! byte_oriented {
            ($($opcode:literal => $op:ident),*$(,)?) => {
                match instruction {
                    $(i if ((i & 0b0011_1111_0000_0000) == (($opcode as u16) << 8)) => Some(Instruction::ByteOriented {
                        op: ByteOrientedOperation::$op,
                        f: RegisterFileAddr((i & 0b0000_0000_0111_1111) as u8),
                        dest: if (i & 0b0000_0000_1000_0000) == 0  { Destination::W } else { Destination::F }
                    }),)*
                    _ => None,
                }
            };
        }
        macro_rules! bit_oriented {
            ($($opcode:literal => $op:ident),*$(,)?) => {
                match instruction {
                    $(i if ((i & 0b0011_1100_0000_0000) == (($opcode as u16) << 8)) => Some(Instruction::BitOriented {
                        op: BitOrientedOperation::$op,
                        b: BitIndex::new(((i & 0b0000_0011_1000_0000) >> 7) as u8),
                        f: RegisterFileAddr::new((i & 0b0000_0000_0111_1111) as u8),
                    }),)*
                    _ => None,
                }
            };
        }
        macro_rules! literal_oriented {
            ($($mask:literal
               $opcode:literal => $op:ident),*$(,)?) => {
                match instruction {
                    $(i if ((i & (($mask as u16) << 8)) == (($opcode as u16) << 8)) => Some(Instruction::LiteralOriented {
                        op: LiteralOrientedOperation::$op,
                        k: (i & 0b0000_0000_1111_1111) as u8,
                    }),)*
                    _ => None,
                }
            };
        }

        let decoded = byte_oriented! {
            0b0000_0111 => AddWf,
            0b0000_0101 => AndWf,
            0b0000_1001 => ComplementF,
            0b0000_0011 => DecrementF,
            0b0000_1011 => DecrementFSkipIfZ,
            0b0000_1010 => IncrementF,
            0b0000_1111 => IncrementFSkipIfZ,
            0b0000_0100 => OrWf,
            0b0000_1000 => MoveF,
            0b0000_1101 => RotateLeftFThroughCarry,
            0b0000_1100 => RotateRightFThroughCarry,
            0b0000_0010 => SubtractWfromF,
            0b0000_1110 => SwapF,
            0b0000_0110 => XorWwithF,
        }
        .or_else(|| {
            bit_oriented! {
                0b0001_0000 => BitClearF,
                0b0001_0100 => BitSetF,
                0b0001_1000 => SkipIfFBitClear,
                0b0001_1100 => SkipIfFBitSet,
            }
        })
        .or_else(|| {
            literal_oriented! {
                0b0011_1100
                0b0011_0000 => MoveLiteralToW,

                0b0011_1110
                0b0011_1110 => AddLiteralToW,

                0b0011_1111
                0b0011_1001 => AndLiteralWithW,

                0b0011_1111
                0b0011_1000 => OrLiteralWithW,

                0b0011_1100
                0b0011_0100 => ReturnWithLiteralInW,

                0b0011_1110
                0b0011_1100 => SubtractWFromLitral,

                0b0011_1111
                0b0011_1010 => XorLiteralWithW,
            }
        })
        .or_else(|| match instruction {
            0b0000_0000_0000_1000 => Some(Instruction::Return),
            0b0000_0000_0110_0100 => Some(Instruction::ClearWatchDogTimer),
            0b0000_0000_0000_1001 => Some(Instruction::ReturnFromInterrupt),
            0b0000_0000_0110_0011 => Some(Instruction::Sleep),
            i if (i & 0b0011_1111_1001_1111) == 0b0000_0000_0000_0000 => Some(Instruction::Noop),
            i if (i & 0b0011_1111_1000_0000) == 0b0000_0001_0000_0000 => Some(Instruction::ClearW),
            i if (i & 0b0011_1000_0000_0000) == 0b0010_1000_0000_0000 => Some(Instruction::Goto {
                addr: ProgramAddr::new(i & 0b0000_0111_1111_1111),
            }),
            i if (i & 0b0011_1000_0000_0000) == 0b0010_0000_0000_0000 => Some(Instruction::Call {
                addr: ProgramAddr::new(i & 0b0000_0111_1111_1111),
            }),
            i if (i & 0b0011_1111_1000_0000) == 0b0000_0001_1000_0000 => {
                Some(Instruction::ClearF {
                    f: RegisterFileAddr::new((i & 0b0000_0000_0111_1111) as u8),
                })
            }
            i if (i & 0b0011_1111_1000_0000) == 0b0000_0000_1000_0000 => {
                Some(Instruction::MoveWtoF {
                    f: RegisterFileAddr::new((i & 0b0000_0000_0111_1111) as u8),
                })
            }
            _ => None,
        });

        println!("{decoded:?}");
    }
}

pub struct RegisterFileAddr(u8);
impl std::fmt::Debug for RegisterFileAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegisterFileAddr({:02x})", self.0)
    }
}
impl RegisterFileAddr {
    pub fn new(addr: u8) -> Self {
        Self(addr)
    }
}

pub struct ProgramAddr(u16);
impl std::fmt::Debug for ProgramAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProgramAddr(0x{:04x})", self.0)
    }
}
impl ProgramAddr {
    pub fn new(addr: u16) -> Self {
        Self(addr)
    }
}

#[derive(Debug)]
pub struct BitIndex(u8);
impl BitIndex {
    pub fn new(i: u8) -> Self {
        assert!(i < 8);
        Self(i)
    }
}

#[derive(Debug)]
pub enum Destination {
    /// Destination is W register
    W,
    /// Destination is the register pointed by f operand
    F,
}

#[derive(Debug)]
enum Instruction {
    ByteOriented {
        op: ByteOrientedOperation,
        f: RegisterFileAddr,
        dest: Destination,
    },

    BitOriented {
        op: BitOrientedOperation,
        b: BitIndex,
        f: RegisterFileAddr,
    },

    LiteralOriented {
        op: LiteralOrientedOperation,
        k: u8,
    },

    /// ```
    /// 0 -> *f, 1 -> Z
    /// ```
    /// - affects: Z
    #[doc(alias = "clrf")]
    ClearF {
        f: RegisterFileAddr,
    },

    /// ```
    /// 0 -> W, 1 -> Z
    /// ```
    /// - affected: Z
    #[doc(alias = "clrw")]
    ClearW,

    /// ```
    /// W -> *f
    /// ```
    /// - affects: None
    #[doc(alias = "movwf")]
    MoveWtoF {
        f: RegisterFileAddr,
    },

    /// ```
    /// no-operation
    /// ```
    /// - affects: None
    Noop,

    /// ```
    /// addr -> PC<10:0>
    /// PCLATH<4:3> -> PC<12:11>
    /// ```
    /// - affects: None
    Goto {
        addr: ProgramAddr,
    },

    /// ```
    /// PC + 1 -> TOS
    /// addr -> PC
    /// PCLATH<4:3> -> PC<12:11>
    /// ```
    /// - affects: None
    /// - cycles: 2
    Call {
        addr: ProgramAddr,
    },

    #[doc(alias = "clrwdt")]
    ClearWatchDogTimer,

    #[doc(alias = "retfie")]
    ReturnFromInterrupt,

    /// ```
    /// TOS -> PC
    /// ```
    /// - cycles: 2
    Return,

    Sleep,
}

#[derive(Debug)]
pub enum ByteOrientedOperation {
    /// ```
    /// W + *f -> destination
    /// ```
    /// - affects: C, DC, Z
    AddWf,

    /// ```
    /// W & *f -> destination
    /// ```
    /// - affects: Z
    AndWf,

    /// ```
    /// Complement f (1's complement?)
    /// ```
    /// - affects: Z
    #[doc(alias = "comf")]
    ComplementF,

    /// ```
    /// *f - 1 -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "decf")]
    DecrementF,

    /// ```
    /// *f - 1 -> destination
    /// if (*f - 1) == 0 {
    ///     nop;
    ///     PC += 1; // skip next instruction
    /// }
    /// ```
    /// - affects: None
    /// - cycles: 2 if *f == 1, otherwise 1
    #[doc(alias = "decfsz")]
    DecrementFSkipIfZ,

    /// *f + 1 -> destination
    /// - affects: Z
    #[doc(alias = "incf")]
    IncrementF,

    /// ```
    /// *f + 1 -> destination
    /// if (*f + 1) == 0 {
    ///     nop;
    ///     PC += 1; // skip next instruction
    /// }
    /// ```
    /// - affects: None
    /// - cycles: 2 if *f == 0xff, otherwise 1
    #[doc(alias = "incfsz")]
    IncrementFSkipIfZ,

    /// ```
    /// W | *f -> W
    /// ```
    /// - affects: Z
    #[doc(alias = "iorwf")]
    OrWf,

    /// ```
    /// *f -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "movf")]
    MoveF,

    /// ```
    /// rotate left F through Carry flag
    ///  <- C <- *f <-
    /// ```
    /// - affects: C
    #[doc(alias = "rlf")]
    RotateLeftFThroughCarry,

    /// ```
    /// rotate right F through Carry flag
    ///  -> C -> *f ->
    /// ```
    /// - affects: C
    #[doc(alias = "rrf")]
    RotateRightFThroughCarry,

    /// ```
    /// *f - W -> destination
    /// ```
    /// - affects: C, DC, Z
    #[doc(alias = "subwf")]
    SubtractWfromF,

    /// ```
    /// *f<3:0> -> destination<7:4>
    /// *f<7:4> -> destination<3:0>
    /// ```
    /// - affects: None
    SwapF,

    /// ```
    /// W ^ *f -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "xorwf")]
    XorWwithF,
}

#[derive(Debug)]
pub enum BitOrientedOperation {
    /// ```
    /// 0 -> f<b>
    /// ```
    /// - affects: None
    #[doc(alias = "bcf")]
    BitClearF,

    /// ```
    /// 1 -> f<b>
    /// ```
    /// - affects: None
    #[doc(alias = "bsf")]
    BitSetF,

    /// ```
    /// if *f<b> == 0 {
    ///     nop;
    ///     PC += 1; // skip next instruction
    /// }
    /// ```
    /// - affects: None
    /// - cycles: 2 if skip, otherwise 1
    #[doc(alias = "btfsc")]
    SkipIfFBitClear,

    /// ```
    /// if *f<b> == 1 {
    ///     nop;
    ///     PC += 1; // skip next instruction
    /// }
    /// ```
    /// - affects: None
    /// - cycles: 2 if skip, otherwise 1
    #[doc(alias = "btfss")]
    SkipIfFBitSet,
}

#[derive(Debug)]
pub enum LiteralOrientedOperation {
    #[doc(alias = "sublw")]
    SubtractWFromLitral,

    #[doc(alias = "xorlw")]
    XorLiteralWithW,

    #[doc(alias = "iorlw")]
    OrLiteralWithW,

    #[doc(alias = "movlw")]
    MoveLiteralToW,

    #[doc(alias = "retlw")]
    ReturnWithLiteralInW,

    #[doc(alias = "addlw")]
    AddLiteralToW,

    #[doc(alias = "andlw")]
    AndLiteralWithW,
}
