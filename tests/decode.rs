use std::io::Cursor;

use stk::inst::{BitIndex, LiteralOrientedInstruction, ProgramAddr};

/// tries to decode
/// ```
/// psect kmain,class=CODE,delta=2
///
/// setup:
///     addwf 0x55, 1
///     andwf 0x55, 0
///     clrf 0x55
///     clrw
///     comf 0x55, 1
///     decf 0x55, 0
///     decfsz 0x55, 1
///     incf 0x55, 0
///     incfsz 0x55, 1
///     iorwf 0x55, 0
///     movf 0x23, 1
///     movwf 0x23
///     nop
///     rlf 0x23, 0
///     rrf 0x23, 1
///     subwf 0x23, 0
///     swapf 0x23, 1
///     xorwf 0x23, 0
///     bcf 0x23, 7
///     bsf 0x23, 4
///     btfsc 0x23, 5
///     btfss 0x55, 1
///     addlw 127
///     andlw 98
///     call subroutine
///     clrwdt
///     goto label1
/// label1:
///     iorlw 34
///     movlw 19
///     retfie ; broken
///     sleep ; broken
///     sublw 45
///     xorlw 12
///
/// subroutine:
///     nop
///     return
///
/// subroutine2:
///     nop
///     retlw 28
///
/// end setup
/// ```

#[test]
fn decode_instructions() {
    let hex_text = ":10000000D5075505D5010301D5095503D50B550A6B
:10001000D50F5504A308A3000000230DA30C230251
:10002000A30E2306A3132316A31AD51C7F3E623901
:10003000212064001B2822381330090063002D3C66
:0A0040000C3A0000080000001C3418
:00000001FF";

    let hex = stk::hex::decode_intel_hex(Cursor::new(hex_text)).unwrap();
    let inst = hex
        .chunks(2)
        .map(|x| {
            let &[a, b] = x else { unreachable!() };
            let code = ((b as u16) << 8) | (a as u16);
            stk::inst::Instruction::from_code(code).unwrap()
        })
        .collect::<Vec<_>>();

    use stk::inst::{
        BitOrientedInstruction, BitOrientedOperation::*, ByteOrientedInstruction,
        ByteOrientedOperation::*, ControlInstruction::*, Destination::*, Instruction::*,
        LiteralOrientedOperation::*, RegisterFileAddr,
    };

    let model = [
        ByteOriented(ByteOrientedInstruction {
            op: AddWf,
            f: RegisterFileAddr(0x55),
            dest: F,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: AndWf,
            f: RegisterFileAddr(0x55),
            dest: W,
        }),
        Control(ClearF {
            f: RegisterFileAddr(0x55),
        }),
        Control(ClearW),
        ByteOriented(ByteOrientedInstruction {
            op: ComplementF,
            f: RegisterFileAddr(0x55),
            dest: F,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: DecrementF,
            f: RegisterFileAddr(0x55),
            dest: W,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: DecrementFSkipIfZ,
            f: RegisterFileAddr(0x55),
            dest: F,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: IncrementF,
            f: RegisterFileAddr(0x55),
            dest: W,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: IncrementFSkipIfZ,
            f: RegisterFileAddr(0x55),
            dest: F,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: OrWf,
            f: RegisterFileAddr(0x55),
            dest: W,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: MoveF,
            f: RegisterFileAddr(0x23),
            dest: F,
        }),
        Control(MoveWtoF {
            f: RegisterFileAddr(0x23),
        }),
        Control(Noop),
        ByteOriented(ByteOrientedInstruction {
            op: RotateLeftFThroughCarry,
            f: RegisterFileAddr(0x23),
            dest: W,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: RotateRightFThroughCarry,
            f: RegisterFileAddr(0x23),
            dest: F,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: SubtractWfromF,
            f: RegisterFileAddr(0x23),
            dest: W,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: SwapF,
            f: RegisterFileAddr(0x23),
            dest: F,
        }),
        ByteOriented(ByteOrientedInstruction {
            op: XorWwithF,
            f: RegisterFileAddr(0x23),
            dest: W,
        }),
        BitOriented(BitOrientedInstruction {
            op: BitClearF,
            b: BitIndex::new(7),
            f: RegisterFileAddr(0x23),
        }),
        BitOriented(BitOrientedInstruction {
            op: BitSetF,
            b: BitIndex::new(4),
            f: RegisterFileAddr(0x23),
        }),
        BitOriented(BitOrientedInstruction {
            op: SkipIfFBitClear,
            b: BitIndex::new(5),
            f: RegisterFileAddr(0x23),
        }),
        BitOriented(BitOrientedInstruction {
            op: SkipIfFBitSet,
            b: BitIndex::new(1),
            f: RegisterFileAddr(0x55),
        }),
        LiteralOriented(LiteralOrientedInstruction {
            op: AddLiteralToW,
            k: 127,
        }),
        LiteralOriented(LiteralOrientedInstruction {
            op: AndLiteralWithW,
            k: 98,
        }),
        Control(Call {
            addr: ProgramAddr(0x0021),
        }),
        Control(ClearWatchDogTimer),
        Control(Goto {
            addr: ProgramAddr(0x001b),
        }),
        LiteralOriented(LiteralOrientedInstruction {
            op: OrLiteralWithW,
            k: 34,
        }),
        LiteralOriented(LiteralOrientedInstruction {
            op: MoveLiteralToW,
            k: 19,
        }),
        Control(ReturnFromInterrupt),
        Control(Sleep),
        LiteralOriented(LiteralOrientedInstruction {
            op: SubtractWFromLitral,
            k: 45,
        }),
        LiteralOriented(LiteralOrientedInstruction {
            op: XorLiteralWithW,
            k: 12,
        }),
        Control(Noop),
        Control(Return),
        Control(Noop),
        LiteralOriented(LiteralOrientedInstruction {
            op: ReturnWithLiteralInW,
            k: 28,
        }),
    ];

    assert_eq!(model.as_slice(), inst);
}
