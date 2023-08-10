use std::fmt::Debug;

use stk_macro::bitmaskeq;

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct RegisterFileAddr(pub u8);
impl std::fmt::Debug for RegisterFileAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegisterFileAddr(0x{:02x})", self.0)
    }
}
impl RegisterFileAddr {
    pub fn new(addr: u8) -> Self {
        Self(addr)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct ProgramAddr(pub u16);
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct BitIndex(pub u8);
impl BitIndex {
    pub fn new(i: u8) -> Self {
        assert!(i < 8);
        Self(i)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Destination {
    /// Destination is W register
    W,
    /// Destination is the register pointed by f operand
    F,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    ByteOriented(ByteOrientedInstruction),
    BitOriented(BitOrientedInstruction),
    LiteralOriented(LiteralOrientedInstruction),
    Control(ControlInstruction),
}

impl Debug for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Instruction::ByteOriented(x) => write!(f, "{x:?}"),
            Instruction::BitOriented(x) => write!(f, "{x:?}"),
            Instruction::LiteralOriented(x) => write!(f, "{x:?}"),
            Instruction::Control(x) => write!(f, "{x:?}"),
        }
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ByteOrientedInstruction {
    pub op: ByteOrientedOperation,
    pub f: RegisterFileAddr,
    pub dest: Destination,
}

impl Debug for ByteOrientedInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: F: 0x{:02x} into {:?}",
            &self.op, self.f.0, &self.dest
        )
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrientedOperation {
    /// ```ignore
    /// W + *f -> destination
    /// ```
    /// - affects: C, DC, Z
    AddWf,

    /// ```ignore
    /// W & *f -> destination
    /// ```
    /// - affects: Z
    AndWf,

    /// ```ignore
    /// Complement f (1's complement?)
    /// ```
    /// - affects: Z
    #[doc(alias = "comf")]
    ComplementF,

    /// ```ignore
    /// *f - 1 -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "decf")]
    DecrementF,

    /// ```ignore
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

    /// ```ignore
    /// *f + 1 -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "incf")]
    IncrementF,

    /// ```ignore
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

    /// ```ignore
    /// W | *f -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "iorwf")]
    OrWf,

    /// ```ignore
    /// *f -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "movf")]
    MoveF,

    /// ```ignore
    /// rotate left F through Carry flag
    ///  <- C <- *f <-
    /// ```
    /// - affects: C
    #[doc(alias = "rlf")]
    RotateLeftFThroughCarry,

    /// ```ignore
    /// rotate right F through Carry flag
    ///  -> C -> *f ->
    /// ```
    /// - affects: C
    #[doc(alias = "rrf")]
    RotateRightFThroughCarry,

    /// ```ignore
    /// *f - W -> destination
    /// ```
    /// - affects: C, DC, Z
    #[doc(alias = "subwf")]
    SubtractWfromF,

    /// ```ignore
    /// *f<3:0> -> destination<7:4>
    /// *f<7:4> -> destination<3:0>
    /// ```
    /// - affects: None
    SwapF,

    /// ```ignore
    /// W ^ *f -> destination
    /// ```
    /// - affects: Z
    #[doc(alias = "xorwf")]
    XorWwithF,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BitOrientedInstruction {
    pub op: BitOrientedOperation,
    pub b: BitIndex,
    pub f: RegisterFileAddr,
}

impl Debug for BitOrientedInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}(0x{:02x}<{}>)", self.op, self.f.0, self.b.0)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitOrientedOperation {
    /// ```ignore
    /// 0 -> f<b>
    /// ```
    /// - affects: None
    #[doc(alias = "bcf")]
    BitClearF,

    /// ```ignore
    /// 1 -> f<b>
    /// ```
    /// - affects: None
    #[doc(alias = "bsf")]
    BitSetF,

    /// ```ignore
    /// if *f<b> == 0 {
    ///     nop;
    ///     PC += 1; // skip next instruction
    /// }
    /// ```
    /// - affects: None
    /// - cycles: 2 if skip, otherwise 1
    #[doc(alias = "btfsc")]
    SkipIfFBitClear,

    /// ```ignore
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LiteralOrientedInstruction {
    pub op: LiteralOrientedOperation,
    pub k: u8,
}

impl Debug for LiteralOrientedInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}({})", &self.op, self.k)
    }
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
            0b0011_1100 => SubtractWFromLiteral,
            0b0011_1111
            0b0011_1010 => XorLiteralWithW,
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralOrientedOperation {
    /// ```ignore
    /// k - W -> W
    /// ```
    /// - affects: C, DC, Z
    #[doc(alias = "sublw")]
    SubtractWFromLiteral,

    /// ```ignore
    /// W ^ k -> W
    /// ```
    /// - affects: Z
    #[doc(alias = "xorlw")]
    XorLiteralWithW,

    /// ```ignore
    /// W | k -> W
    /// ```
    /// - affects: Z
    #[doc(alias = "iorlw")]
    OrLiteralWithW,

    /// ```ignore
    /// k -> W
    /// ```
    /// - affects: None
    #[doc(alias = "movlw")]
    MoveLiteralToW,

    /// ```ignore
    /// k -> W
    /// TOS -> PC
    /// ```
    /// - affects: None
    #[doc(alias = "retlw")]
    ReturnWithLiteralInW,

    /// ```ignore
    /// W + k -> W
    /// ```
    /// - affects: None
    #[doc(alias = "addlw")]
    AddLiteralToW,

    /// ```ignore
    /// W & k -> W
    /// ```
    /// - affects: None
    #[doc(alias = "andlw")]
    AndLiteralWithW,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ControlInstruction {
    /// ```ignore
    /// 0 -> WDT
    /// 0 -> WDT prescaler
    /// 1 -> TO
    /// 1 -> PD
    /// ```
    /// - affects: TO, PD
    #[doc(alias = "clrwdt")]
    ClearWatchDogTimer,

    /// ```ignore
    /// TOS -> PC
    /// 1 -> GIE
    /// ```
    /// - affects: None
    /// - cycles: 2
    #[doc(alias = "retfie")]
    ReturnFromInterrupt,

    /// ```ignore
    /// TOS -> PC
    /// ```
    /// - affects: None
    /// - cycles: 2
    Return,

    /// ```ignore
    /// 0 -> WDT prescaler
    /// 1 -> TO
    /// 0 -> PD
    /// ```
    /// - affects: TO, PD
    Sleep,

    /// ```ignore
    /// no-operation
    /// ```
    /// - affects: None
    #[doc(alias = "nop")]
    Noop,

    /// ```ignore
    /// addr -> PC<10:0>
    /// PCLATH<4:3> -> PC<12:11>
    /// ```
    /// - affects: None
    Goto { addr: ProgramAddr },

    /// ```ignore
    /// PC + 1 -> TOS
    /// addr -> PC
    /// PCLATH<4:3> -> PC<12:11>
    /// ```
    /// - affects: None
    /// - cycles: 2
    Call { addr: ProgramAddr },

    /// ```ignore
    /// 0 -> *f, 1 -> Z
    /// ```
    /// - affects: Z
    #[doc(alias = "clrf")]
    ClearF { f: RegisterFileAddr },

    /// ```ignore
    /// 0 -> W, 1 -> Z
    /// ```
    /// - affected: Z
    #[doc(alias = "clrw")]
    ClearW,

    /// ```ignore
    /// W -> *f
    /// ```
    /// - affects: None
    #[doc(alias = "movwf")]
    MoveWtoF { f: RegisterFileAddr },
}

impl Debug for ControlInstruction {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClearWatchDogTimer => write!(fmt, "ClearWatchDogTimer"),
            Self::ReturnFromInterrupt => write!(fmt, "ReturnFromInterrupt"),
            Self::Return => write!(fmt, "Return"),
            Self::Sleep => write!(fmt, "Sleep"),
            Self::Noop => write!(fmt, "Noop"),
            Self::Goto { addr } => write!(fmt, "Goto(0x{:04x})", addr.0),
            Self::Call { addr } => write!(fmt, "Call(0x{:04x})", addr.0),
            Self::ClearF { f } => write!(fmt, "ClearF(0x{:02x})", f.0),
            Self::ClearW => write!(fmt, "ClearW"),
            Self::MoveWtoF { f } => write!(fmt, "MoveWtoF(0x{:02x})", f.0),
        }
    }
}

impl ControlInstruction {
    pub fn from_code(i: u16) -> Option<ControlInstruction> {
        bitmaskeq! {
            match i {
                0b0000_0000_0000_1000 => Some(ControlInstruction::Return),
                0b0000_0000_0110_0100 => Some(ControlInstruction::ClearWatchDogTimer),
                0b0000_0000_0000_1001 => Some(ControlInstruction::ReturnFromInterrupt),
                0b0000_0000_0110_0011 => Some(ControlInstruction::Sleep),
                m_xx00_0000_0xx0_0000 => Some(ControlInstruction::Noop),
                m_xx00_0001_0xxx_xxxx => Some(ControlInstruction::ClearW),
                m_xx10_1aaa_aaaa_aaaa => Some(ControlInstruction::Goto { addr: ProgramAddr::new(a) }),
                m_xx10_0aaa_aaaa_aaaa => Some(ControlInstruction::Call { addr: ProgramAddr::new(a) }),
                m_xx00_0001_1fff_ffff => Some(ControlInstruction::ClearF { f: RegisterFileAddr::new(f as u8) }),
                m_xx00_0000_1fff_ffff => Some(ControlInstruction::MoveWtoF { f: RegisterFileAddr::new(f as u8) }),
                _ => None,
            }
        }
    }
}
