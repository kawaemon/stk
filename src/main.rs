use std::{cell::RefCell, fs::File, io::BufReader, rc::Rc, time::Instant};

use stk::{
    hex::decode_intel_hex,
    inst::Instruction,
    vm::{reg::Register, Ticker, P16F88},
};

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    let mut flash = decode_intel_hex(BufReader::new(File::open("./main.hex").unwrap())).unwrap();

    for instruction in flash.chunks(2) {
        let &[a, b] = instruction else {
            unreachable!()
        };

        let instruction = ((b as u16) << 8) | (a as u16);
        let decoded = Instruction::from_code(instruction);
        println!("{decoded:?}");
    }

    if flash.len() > 7168 {
        panic!("program is too large");
    }
    flash.resize(7168, 0);

    const CLOCKS_PER_SEC: u128 = 20_000_000;
    const CLOCKS_PER_CYCLE: u128 = 4;

    #[derive(Default, Debug)]
    struct LocalTickerInner {
        cycles: u128,
        records: Vec<(Instant, u8)>,
    }
    impl Ticker for LocalTickerInner {
        fn tick(&mut self, reg: &stk::vm::reg::Registers, cycles: u8) {
            self.cycles += CLOCKS_PER_CYCLE * cycles as u128;

            let this_time = reg.special.porta().read();
            if let Some((_, last)) = self.records.last() {
                if *last == this_time {
                    return;
                }
            }

            self.records.push((Instant::now(), this_time));
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
        if RefCell::borrow(&ticker.0).cycles > CLOCKS_PER_SEC / CLOCKS_PER_CYCLE * 5 {
            break;
        }
    }

    println!("{:#?}", RefCell::borrow(&ticker.0));
}
