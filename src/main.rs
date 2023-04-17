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
        let decoded = Instruction::from_code(instruction);
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
pub enum Instruction {
    ByteOriented(ByteOrientedInstruction),
    BitOriented(BitOrientedInstruction),
    LiteralOriented(LiteralOrientedInstruction),
    Control(ControlInstruction),
}

impl Instruction {
    pub fn from_code(i: u16) -> Option<Instruction> {
        ByteOrientedInstruction::from_code(i)
            .map(Instruction::ByteOriented)
            .or(BitOrientedInstruction::from_code(i).map(Instruction::BitOriented))
            .or(LiteralOrientedInstruction::from_code(i).map(Instruction::LiteralOriented))
            .or(ControlInstruction::from_code(i).map(Instruction::Control))
    }
}

#[derive(Debug)]
pub struct ByteOrientedInstruction {
    op: ByteOrientedOperation,
    f: RegisterFileAddr,
    dest: Destination,
}

impl ByteOrientedInstruction {
    pub fn from_code(i: u16) -> Option<ByteOrientedInstruction> {
        macro_rules! byte_oriented {
            ($($opcode:literal => $op:ident),*$(,)?) => {
                $(
                    if ((i & 0b0011_1111_0000_0000) == (($opcode as u16) << 8)) {
                        return Some(ByteOrientedInstruction {
                            op: ByteOrientedOperation::$op,
                            f: RegisterFileAddr((i & 0b0000_0000_0111_1111) as u8),
                            dest: if (i & 0b0000_0000_1000_0000) == 0 { Destination::W } else { Destination::F }
                        })
                    }
                )*
            };
        }
        byte_oriented! {
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

        None
    }
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

    /// ```
    /// *f + 1 -> destination
    /// ```
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
pub struct BitOrientedInstruction {
    op: BitOrientedOperation,
    b: BitIndex,
    f: RegisterFileAddr,
}

impl BitOrientedInstruction {
    pub fn from_code(i: u16) -> Option<BitOrientedInstruction> {
        macro_rules! bit_oriented {
            ($($opcode:literal => $op:ident),*$(,)?) => {
                $(
                    if ((i & 0b0011_1100_0000_0000) == (($opcode as u16) << 8)) {
                        return Some(BitOrientedInstruction {
                            op: BitOrientedOperation::$op,
                            b: BitIndex::new(((i & 0b0000_0011_1000_0000) >> 7) as u8),
                            f: RegisterFileAddr::new((i & 0b0000_0000_0111_1111) as u8),
                        });
                    }
                )*
            };
        }
        bit_oriented! {
            0b0001_0000 => BitClearF,
            0b0001_0100 => BitSetF,
            0b0001_1000 => SkipIfFBitClear,
            0b0001_1100 => SkipIfFBitSet,
        }

        None
    }
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
pub struct LiteralOrientedInstruction {
    op: LiteralOrientedOperation,
    k: u8,
}

impl LiteralOrientedInstruction {
    pub fn from_code(i: u16) -> Option<LiteralOrientedInstruction> {
        macro_rules! literal_oriented {
            ($($mask:literal
               $opcode:literal => $op:ident),*$(,)?) => {
                $(
                    if ((i & (($mask as u16) << 8)) == (($opcode as u16) << 8)) {
                        return Some(LiteralOrientedInstruction {
                            op: LiteralOrientedOperation::$op,
                            k: (i & 0b0000_0000_1111_1111) as u8,
                        });
                    }
                )*
            };
        }

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

        None
    }
}

#[derive(Debug)]
pub enum LiteralOrientedOperation {
    /// ```
    /// k - W -> W
    /// ```
    /// - affects: C, DC, Z
    #[doc(alias = "sublw")]
    SubtractWFromLitral,

    /// ```
    /// W ^ k -> W
    /// ```
    /// - affects: Z
    #[doc(alias = "xorlw")]
    XorLiteralWithW,

    /// ```
    /// W | k -> W
    /// ```
    /// - affects: Z
    #[doc(alias = "iorlw")]
    OrLiteralWithW,

    /// ```
    /// k -> W
    /// ```
    /// - affects: None
    #[doc(alias = "movlw")]
    MoveLiteralToW,

    /// ```
    /// k -> W
    /// TOS -> PC
    /// ```
    /// - affects: None
    #[doc(alias = "retlw")]
    ReturnWithLiteralInW,

    /// ```
    /// W + k -> W
    /// ```
    /// - affects: None
    #[doc(alias = "addlw")]
    AddLiteralToW,

    /// ```
    /// W & k -> W
    /// ```
    /// - affects: None
    #[doc(alias = "andlw")]
    AndLiteralWithW,
}

#[derive(Debug)]
pub enum ControlInstruction {
    /// ```
    /// 0 -> WDT
    /// 0 -> WDT prescaler
    /// 1 -> TO
    /// 1 -> PD
    /// ```
    /// - affects: TO, PD
    #[doc(alias = "clrwdt")]
    ClearWatchDogTimer,

    /// ```
    /// TOS -> PC
    /// 1 -> GIE
    /// ```
    /// - affects: None
    /// - cycles: 2
    #[doc(alias = "retfie")]
    ReturnFromInterrupt,

    /// ```
    /// TOS -> PC
    /// ```
    /// - affects: None
    /// - cycles: 2
    Return,

    /// ```
    /// 0 -> WDT prescaler
    /// 1 -> TO
    /// 0 -> PD
    /// ```
    /// - affects: TO, PD
    Sleep,

    /// ```
    /// no-operation
    /// ```
    /// - affects: None
    #[doc(alias = "nop")]
    Noop,

    /// ```
    /// addr -> PC<10:0>
    /// PCLATH<4:3> -> PC<12:11>
    /// ```
    /// - affects: None
    Goto { addr: ProgramAddr },

    /// ```
    /// PC + 1 -> TOS
    /// addr -> PC
    /// PCLATH<4:3> -> PC<12:11>
    /// ```
    /// - affects: None
    /// - cycles: 2
    Call { addr: ProgramAddr },

    /// ```
    /// 0 -> *f, 1 -> Z
    /// ```
    /// - affects: Z
    #[doc(alias = "clrf")]
    ClearF { f: RegisterFileAddr },

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
    MoveWtoF { f: RegisterFileAddr },
}

impl ControlInstruction {
    pub fn from_code(i: u16) -> Option<ControlInstruction> {
        match i {
            0b0000_0000_0000_1000 => Some(ControlInstruction::Return),
            0b0000_0000_0110_0100 => Some(ControlInstruction::ClearWatchDogTimer),
            0b0000_0000_0000_1001 => Some(ControlInstruction::ReturnFromInterrupt),
            0b0000_0000_0110_0011 => Some(ControlInstruction::Sleep),
            i if (i & 0b0011_1111_1001_1111) == 0b0000_0000_0000_0000 => {
                Some(ControlInstruction::Noop)
            }
            i if (i & 0b0011_1111_1000_0000) == 0b0000_0001_0000_0000 => {
                Some(ControlInstruction::ClearW)
            }
            i if (i & 0b0011_1000_0000_0000) == 0b0010_1000_0000_0000 => {
                Some(ControlInstruction::Goto {
                    addr: ProgramAddr::new(i & 0b0000_0111_1111_1111),
                })
            }
            i if (i & 0b0011_1000_0000_0000) == 0b0010_0000_0000_0000 => {
                Some(ControlInstruction::Call {
                    addr: ProgramAddr::new(i & 0b0000_0111_1111_1111),
                })
            }
            i if (i & 0b0011_1111_1000_0000) == 0b0000_0001_1000_0000 => {
                Some(ControlInstruction::ClearF {
                    f: RegisterFileAddr::new((i & 0b0000_0000_0111_1111) as u8),
                })
            }
            i if (i & 0b0011_1111_1000_0000) == 0b0000_0000_1000_0000 => {
                Some(ControlInstruction::MoveWtoF {
                    f: RegisterFileAddr::new((i & 0b0000_0000_0111_1111) as u8),
                })
            }
            _ => None,
        }
    }
}
