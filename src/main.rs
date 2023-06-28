use std::{cell::RefCell, fs::File, io::BufReader, rc::Rc, time::Duration};

use stk::{
    hex::decode_intel_hex,
    vm::{reg::Register, Ticker, P16F88},
};

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    let mut flash = decode_intel_hex(BufReader::new(File::open("./main.hex").unwrap())).unwrap();

    // for (i, instruction) in flash.chunks(2).enumerate() {
    //     let &[a, b] = instruction else {
    //         unreachable!()
    //     };

    //     let instruction = ((b as u16) << 8) | (a as u16);
    //     let decoded = inst::Instruction::from_code(instruction);
    //     println!("0x{:x}: {decoded:?}", i * 2);
    // }

    if flash.len() > 7168 {
        panic!("program is too large");
    }
    flash.resize(7168, 0);

    const CLOCKS_PER_SEC: u128 = 20_000_000;
    const CLOCKS_PER_CYCLE: u128 = 4;

    #[derive(Default, Debug)]
    struct LocalTickerInner {
        clocks: u128,
        records: Vec<(u128, u8)>,
    }
    impl Ticker for LocalTickerInner {
        fn tick(&mut self, reg: &stk::vm::reg::Registers, cycles: u8) {
            self.clocks += CLOCKS_PER_CYCLE * cycles as u128;

            let this_time = reg.special.porta().read();
            if let Some((_, last)) = self.records.last() {
                if *last == this_time {
                    return;
                }
            }

            self.records.push((self.clocks, this_time));
        }
    }
    #[derive(Default, Clone)]
    struct LocalTicker(Rc<RefCell<LocalTickerInner>>);
    impl Ticker for LocalTicker {
        fn tick(&mut self, reg: &stk::vm::reg::Registers, cycles: u8) {
            self.0.borrow_mut().tick(reg, cycles);
        }
    }

    let ticker = LocalTicker::default();
    let mut vm = P16F88::new(flash.try_into().unwrap(), ticker.clone());
    loop {
        vm.step();
        if RefCell::borrow(&ticker.0).clocks > CLOCKS_PER_SEC * 3 {
            break;
        }
    }

    let d = RefCell::borrow(&ticker.0);
    let mut before = None;
    for (d, b) in &d.records {
        let dh = Duration::from_secs_f64(*d as f64 / CLOCKS_PER_SEC as f64);
        print!("{dh:?}({d})");
        if let Some(before) = before {
            let d = d - before;
            let dh = Duration::from_secs_f64(d as f64 / CLOCKS_PER_SEC as f64);
            print!(" (diff: {dh:?}({d}))");
        }
        println!(": 0b{b:08b}");
        before = Some(d);
    }
}
