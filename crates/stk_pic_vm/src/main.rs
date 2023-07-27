use std::{
    cell::RefCell, fmt::Debug, fs::File, io::BufReader, path::PathBuf, rc::Rc, time::Duration,
};

use stk_pic_vm::{
    hex::decode_intel_hex,
    vm::p16f88::{reg::Registers, Ticker, P16F88},
};

use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    file: PathBuf,
}

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Args::parse();

    let mut flash = decode_intel_hex(BufReader::new(File::open(args.file).unwrap())).unwrap();

    if flash.len() > 7168 {
        log::warn!("program is too large");
    }
    flash.resize(7168, 0);

    const CLOCKS_PER_SEC: u128 = 20_000_000;
    const CLOCKS_PER_CYCLE: u128 = 4;

    trait RecordPredicate {
        type Record: Debug;
        fn record(&mut self, reg: &stk_pic_vm::vm::p16f88::reg::Registers) -> Option<Self::Record>;
    }
    struct HD44780Record {
        e: bool,
        rs: bool,
        db: u8,
    }
    impl Debug for HD44780Record {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "HD44780Record(e: {}, rs: {}, db: {:04b})",
                self.e, self.rs, self.db
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
            reg.special.portb().0
        }
    }
    impl RecordPredicate for HD44780DebugPredicate {
        type Record = HD44780Record;
        fn record(&mut self, reg: &Registers) -> Option<HD44780Record> {
            let rec = match (self.before_e, Self::e(reg)) {
                // 立ち下がりエッジ
                (true, false) => Some(HD44780Record {
                    e: Self::e(reg),
                    rs: Self::rs(reg),
                    db: Self::db(reg),
                }),
                // 立ち上がりエッジ
                (false, true) | (true, true) | (false, false) => None,
            };
            self.before_e = Self::e(reg);
            rec
        }
    }

    #[derive(Default, Debug)]
    struct LocalTickerInner<R: RecordPredicate> {
        clocks: u128,
        records: Vec<(u128, R::Record)>,
        pred: R,
    }
    impl<R: RecordPredicate> Ticker for LocalTickerInner<R> {
        fn tick(&mut self, reg: &Registers, cycles: u8) {
            self.clocks += CLOCKS_PER_CYCLE * cycles as u128;
            if let Some(record) = self.pred.record(reg) {
                self.records.push((self.clocks, record));
            }
        }
    }
    struct LocalTicker<R: RecordPredicate>(Rc<RefCell<LocalTickerInner<R>>>);
    impl<R: RecordPredicate> Clone for LocalTicker<R> {
        fn clone(&self) -> Self {
            Self(Rc::clone(&self.0))
        }
    }
    impl<R: RecordPredicate> LocalTicker<R> {
        fn new(pred: R) -> Self {
            LocalTicker(Rc::new(RefCell::new(LocalTickerInner {
                clocks: 0,
                records: vec![],
                pred,
            })))
        }
    }
    impl<R: RecordPredicate> Ticker for LocalTicker<R> {
        fn tick(&mut self, reg: &Registers, cycles: u8) {
            self.0.borrow_mut().tick(reg, cycles);
        }
    }

    let ticker = LocalTicker::new(HD44780DebugPredicate::new());
    let mut vm = P16F88::new(flash.try_into().unwrap(), ticker.clone());
    loop {
        vm.step();
        // if RefCell::borrow(&ticker.0).clocks > CLOCKS_PER_SEC * 3 {
        //     break;
        // }
        if dbg!(vm.pc()) > 7000 {
            break;
        }
    }

    let data = RefCell::borrow(&ticker.0);
    let mut before = None;
    for (clock, record) in &data.records {
        let duration = Duration::from_secs_f64(*clock as f64 / CLOCKS_PER_SEC as f64);
        print!("{duration:04.02?}({clock})");
        if let Some(before) = before {
            let d = clock - before;
            let dh = Duration::from_secs_f64(d as f64 / CLOCKS_PER_SEC as f64);
            print!(" (diff: {dh:04.02?}({d}))");
        }
        println!(": {record:?}");
        before = Some(clock);
    }
}
