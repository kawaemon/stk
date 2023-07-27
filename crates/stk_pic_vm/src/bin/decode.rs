use std::{fmt::Debug, fs::File, io::BufReader, path::PathBuf};

use clap::Parser;
use stk_pic_vm::{
    inst::{BitOrientedInstruction, ByteOrientedInstruction, ControlInstruction, Instruction},
    vm::p16f88,
};

#[derive(Parser, Debug)]
struct Args {
    file: PathBuf,
}

fn format_instruction(inst: Instruction) -> String {
    match inst {
        Instruction::ByteOriented(ByteOrientedInstruction { op, f, dest }) => {
            let name = p16f88::register_name_at(f).join(", ");
            format!("{:?}: 0x{:02x}({name}) into {:?}", op, f.0, dest)
        }

        Instruction::BitOriented(BitOrientedInstruction { op, b, f }) => {
            let name = p16f88::register_name_at(f).join(", ");
            format!("{:?}(0x{:02x}({})<{}>)", op, f.0, name, b.0)
        }

        l @ Instruction::LiteralOriented(_) => format!("{l:?}"),

        o @ Instruction::Control(c) => match c {
            ControlInstruction::ClearF { f } => format!(
                "ClearF(0x{:02x}({}))",
                f.0,
                p16f88::register_name_at(f).join(", ")
            ),
            ControlInstruction::MoveWtoF { f } => format!(
                "MoveWtoF(0x{:02x}({}))",
                f.0,
                p16f88::register_name_at(f).join(", ")
            ),
            _ => format!("{o:?}"),
        },
    }
}

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Args::parse();
    let flash =
        stk_pic_vm::hex::decode_intel_hex(BufReader::new(File::open(args.file).unwrap())).unwrap();

    let mut noop = None;

    for (i, instruction) in flash.chunks(2).enumerate() {
        let &[a, b] = instruction else { unreachable!() };

        let instruction = ((b as u16) << 8) | (a as u16);
        let decoded = stk_pic_vm::inst::Instruction::from_code(instruction);

        match decoded {
            Some(Instruction::Control(ControlInstruction::Noop)) => {
                if noop.is_none() {
                    noop = Some(i);
                }
            }

            Some(d) => {
                if let Some(oi) = noop.take() {
                    let diff = oi.abs_diff(i);
                    let count = diff / 2;

                    if count > 4 {
                        println!("0x{:x}..0x{:x}: {} Noops", oi, i, diff / 2);
                    } else {
                        for i in (0..count + 1).map(|x| oi + x) {
                            println!("0x{i:04x}(0x00): Noop");
                        }
                    }
                }

                println!("0x{:04x}({instruction:04x}): {}", i, format_instruction(d));
            }

            None => {}
        }
    }
}
