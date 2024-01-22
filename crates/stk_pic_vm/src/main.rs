use std::fmt::Debug;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use stk_hd44780_vm::{Hd44780, Hd44780PinState, PinObserver};
use stk_pic_vm::hex::decode_intel_hex;
use stk_pic_vm::vm::p16f88::reg::Registers;
use stk_pic_vm::vm::p16f88::{Ticker, P16F88};

#[derive(Parser, Debug)]
struct Args {
    file: PathBuf,
}

fn main() {
    tracing_subscriber::fmt()
        .with_ansi(std::env::var("NO_COLOR").is_err())
        .init();

    let args = Args::parse();

    let mut flash = decode_intel_hex(BufReader::new(File::open(args.file).unwrap())).unwrap();

    if flash.len() > 7168 {
        tracing::warn!(
            "program is too large; expected: {}, actual: {}",
            7168,
            flash.len()
        );
    }
    flash.resize(7168, 0);

    const CLOCKS_PER_SEC: u128 = 20_000_000;
    const CLOCKS_PER_CYCLE: u128 = 4;

    trait RecordPredicate {
        type Record: Debug;
        fn record(&mut self, vm: &P16F88) -> Option<Self::Record>;
    }
    struct HD44780Record {
        e: bool,
        rs: bool,
        db: u8,
        callstack: Vec<u16>,
    }
    impl Debug for HD44780Record {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "HD44780Record(e: {}, rs: {}, db: {:#010b}, cs: {})",
                self.e,
                self.rs,
                self.db,
                self.callstack
                    .iter()
                    .map(|x| format!("{x:#06x}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
    struct HD44780DebugPredicate {
        before_e: bool,
    }
    impl HD44780DebugPredicate {
        fn new() -> Self {
            Self { before_e: false }
        }
        fn e(reg: &Registers) -> bool {
            (reg.special.porta().0 & 0b0000_1000) != 0
        }
        fn rs(reg: &Registers) -> bool {
            (reg.special.porta().0 & 0b0001_0000) != 0
        }
        fn db(reg: &Registers) -> u8 {
            reg.special.portb().0 << 4
        }
    }
    impl RecordPredicate for HD44780DebugPredicate {
        type Record = HD44780Record;
        fn record(&mut self, vm: &P16F88) -> Option<HD44780Record> {
            let reg = &vm.register;
            let rec = match (self.before_e, Self::e(reg)) {
                // 立ち下がりエッジ
                (true, false) => Some(HD44780Record {
                    e: Self::e(reg),
                    rs: Self::rs(reg),
                    db: Self::db(reg),
                    callstack: vm.call_stack.to_vec(),
                }),
                // 立ち上がりエッジ
                (false, true) => None,
                // 状態変化なし
                (true, true) | (false, false) => None,
            };
            self.before_e = Self::e(reg);
            if let Some(rec) = rec.as_ref() {
                tracing::info!("{rec:?}");
            }
            rec
        }
    }

    #[derive(Debug)]
    struct TickerRecord<R> {
        clock: u128,
        pc: u16,
        record: R,
    }
    #[derive(Default, Debug)]
    struct LocalTickerInner<R: RecordPredicate> {
        clock: u128,
        records: Vec<TickerRecord<R::Record>>,
        pred: R,
        lcd: Hd44780,
    }
    impl<R: RecordPredicate> Ticker for LocalTickerInner<R> {
        fn tick(&mut self, vm: &P16F88, cycles: u8) {
            self.clock += CLOCKS_PER_CYCLE * cycles as u128;
            if let Some(record) = self.pred.record(vm) {
                let record = TickerRecord { clock: self.clock, pc: vm.pc(), record };
                self.records.push(record);
            }
            let reg = &vm.register;
            let db = HD44780DebugPredicate::db(reg);
            self.lcd.update(Hd44780PinState {
                rs: Some(HD44780DebugPredicate::rs(reg)),
                rw: Some(false), // TODO: 確認
                e: Some(HD44780DebugPredicate::e(reg)),
                db7: Some((db & (1 << 7)) != 0),
                db6: Some((db & (1 << 6)) != 0),
                db5: Some((db & (1 << 5)) != 0),
                db4: Some((db & (1 << 4)) != 0),
                db3: None,
                db2: None,
                db1: None,
                db0: None,
            })
        }
    }

    let mut ticker = LocalTickerInner {
        clock: 0,
        records: vec![],
        pred: HD44780DebugPredicate::new(),
        lcd: Hd44780::new(),
    };

    let mut vm = P16F88::new(flash.try_into().unwrap());
    loop {
        vm.step(&mut ticker);
        if vm.pc() * 2 > 7000 {
            break;
        }
    }

    let mut before = None;
    for TickerRecord { clock, pc, record } in &ticker.records {
        let duration = Duration::from_secs_f64(*clock as f64 / CLOCKS_PER_SEC as f64);
        print!("{duration:04.02?} clk: {clock}, pc: {pc:#x}");
        if let Some(before) = before {
            let d = clock - before;
            let dh = Duration::from_secs_f64(d as f64 / CLOCKS_PER_SEC as f64);
            print!(" (diff: {dh:04.02?}({d}))");
        }
        println!(": {record:?}");
        before = Some(clock);
    }
}
