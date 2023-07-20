use arrayvec::ArrayVec;

use crate::{
    inst::{
        BitOrientedInstruction, BitOrientedOperation, ByteOrientedInstruction,
        ByteOrientedOperation, ControlInstruction, Destination, Instruction,
        LiteralOrientedInstruction, LiteralOrientedOperation,
    },
    vm::reg::Register,
};

// datasheets:
//   - https://ww1.microchip.com/downloads/aemDocuments/documents/MCU08/ProductDocuments/DataSheets/30487D.pdf
//   - https://ww1.microchip.com/downloads/en/DeviceDoc/31029a.pdf

pub struct P16F88<T: Ticker> {
    w: u8,
    pc: u16,
    flash: [u8; 7168],
    call_stack: ArrayVec<u16, 8>,
    register: reg::Registers,
    ticker: T,
}

pub trait Ticker {
    fn tick(&mut self, reg: &reg::Registers, cycles: u8);
}

impl<T: Ticker> P16F88<T> {
    #[allow(clippy::new_without_default)]
    pub fn new(flash: [u8; 7168], ticker: T) -> Self {
        P16F88 {
            w: 0,
            pc: 0,
            flash,
            call_stack: ArrayVec::new(),
            register: reg::Registers::new(),
            ticker,
        }
    }

    pub fn step(&mut self) {
        let a = self.flash[self.pc as usize];
        let b = self.flash[(self.pc + 1) as usize];
        let bytecode = ((b as u16) << 8) | (a as u16);
        let inst =
            Instruction::from_code(bytecode).expect("couldn't decode bytecode into instruction");
        self.exec(inst);
    }

    fn dc(a: u8, b: u8) -> bool {
        // https://en.wikipedia.org/wiki/Carry-lookahead_adder
        let at = |x, i| (x & (1u8 << i)) != 0u8;
        let g = |i| at(a, i) & at(b, i);
        let p = |i| at(a, i) | at(b, i);
        g(3) | (g(2) & p(3)) | (g(1) & p(2) & p(3)) | (g(0) & p(1) & p(2) & p(3))
        // | (false & p(0) & p(1) & p(2) & p(3))
    }

    pub fn exec(&mut self, inst: Instruction) {
        use BitOrientedInstruction as B;
        use BitOrientedOperation::*;
        use ByteOrientedInstruction as Y;
        use ByteOrientedOperation::*;
        use ControlInstruction::*;
        use Instruction::*;
        use LiteralOrientedInstruction as L;
        use LiteralOrientedOperation::*;

        macro_rules! gen {
            (@lit $op:expr) => {
                $op;
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            };

            (@byte $f:ident, $d:ident, |$r:ident| $op:expr) => {
                match $d {
                    Destination::W => {
                        let $r = self.register.at($f).read();
                        self.w = $op;
                    }
                    Destination::F => {
                        let $r = self.register.at($f).read();
                        let res = $op;
                        self.register.at($f).write(res);
                    }
                }
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            };
        }

        // TODO: overflow check
        // TODO: status flags

        match inst {
            ByteOriented(Y { op: AddWf, f, dest }) => {
                gen!(@byte f, dest, |b| {
                    let a = self.w;
                    let (ret, overflow) = a.overflowing_add(b);
                    let st = self.register.special().status_mut();
                    st.set(reg::STATUS::Z, ret == 0);
                    st.set(reg::STATUS::C, overflow);
                    st.set(reg::STATUS::DC, Self::dc(a, b));
                    ret
                });
            }
            ByteOriented(Y { op: AndWf, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let ret = self.w & x;
                    self.register.special().status_mut().set(reg::STATUS::Z, ret == 0);
                    ret
                });
            }
            ByteOriented(Y { op: ComplementF, f, dest }) => {
                // read: datasheets[1] P20
                gen!(@byte f, dest, |x| {
                    let ret = !x;
                    self.register.special().status_mut().set(reg::STATUS::Z, ret == 0);
                    ret
                });
            }
            ByteOriented(Y { op: DecrementF, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let ret = x.wrapping_add(1);
                    self.register.special().status_mut().set(reg::STATUS::Z, ret == 0);
                    ret
                });
            }
            ByteOriented(Y { op: DecrementFSkipIfZ, f, dest }) => {
                let ret = self.register.at(f).read().wrapping_sub(1);
                match dest {
                    Destination::W => self.w = ret,
                    Destination::F => self.register.at(f).write(ret),
                }
                let skip = ret == 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(&self.register, if skip { 2 } else { 1 });
            }
            ByteOriented(Y { op: IncrementF, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let ret = x.wrapping_add(1);
                    self.register.special().status_mut().set(reg::STATUS::Z, ret == 0);
                    ret
                });
            }
            ByteOriented(Y { op: IncrementFSkipIfZ, f, dest }) => {
                let res = self.register.at(f).read().wrapping_add(1);
                match dest {
                    Destination::W => self.w = res,
                    Destination::F => self.register.at(f).write(res),
                }
                let skip = res == 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(&self.register, if skip { 2 } else { 1 });
            }
            ByteOriented(Y { op: OrWf, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let ret = self.w | x;
                    self.register.special().status_mut().set(reg::STATUS::Z, ret == 0);
                    ret
                });
            }
            ByteOriented(Y { op: MoveF, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let ret = x;
                    self.register.special().status_mut().set(reg::STATUS::Z, ret == 0);
                    ret
                });
            }
            ByteOriented(Y { op: RotateLeftFThroughCarry, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let status = self.register.special().status_mut();

                    let f_msb = (x & 0b1000_0000) != 0;
                    let mut ret = x << 1;
                    if status.contains(reg::STATUS::C) {
                        ret |= 1;
                    }
                    status.set(reg::STATUS::C, f_msb);

                    ret
                });
            }
            ByteOriented(Y { op: RotateRightFThroughCarry, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let status = self.register.special().status_mut();

                    let f_lsb = (x & 0b000_0001) != 0;
                    let mut ret = x >> 1;
                    if status.contains(reg::STATUS::C) {
                        ret |= 0b1000_0000;
                    }
                    status.set(reg::STATUS::C, f_lsb);

                    ret
                });
            }
            ByteOriented(Y { op: SubtractWfromF, f, dest }) => {
                gen!(@byte f, dest, |b| {
                    let a = self.w;
                    let (ret, overflow) = a.overflowing_sub(b);
                    let st = self.register.special().status_mut();
                    st.set(reg::STATUS::Z, ret == 0);
                    st.set(reg::STATUS::C, overflow);
                    st.set(reg::STATUS::DC, Self::dc(a, (!b).wrapping_add(1)));
                    ret
                });
            }
            ByteOriented(Y { op: SwapF, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let left = x & 0b1111_0000;
                    let right = x & 0b0000_1111;
                    (right << 4) | (left >> 4)
                });
            }
            ByteOriented(Y { op: XorWwithF, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let ret = self.w ^ x;
                    self.register.special().status_mut().set(reg::STATUS::Z, ret == 0);
                    ret
                });
            }
            BitOriented(B { op: BitClearF, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                self.register.at(f).write_with(&|x| x & (!mask));
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            }
            BitOriented(B { op: BitSetF, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                self.register.at(f).write_with(&|x| x | mask);
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            }
            BitOriented(B { op: SkipIfFBitClear, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                let skip = (self.register.at(f).read() & mask) == 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(&self.register, if skip { 2 } else { 1 });
            }
            BitOriented(B { op: SkipIfFBitSet, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                let skip = (self.register.at(f).read() & mask) != 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(&self.register, if skip { 2 } else { 1 });
            }
            LiteralOriented(L { op: SubtractWFromLiteral, k }) => {
                gen!(@lit {
                    let a = k;
                    let b = (!self.w).wrapping_add(1);
                    let (res, overflow) = k.overflowing_add(b);
                    let st = self.register.special().status_mut();
                    st.set(reg::STATUS::Z, self.w == 0);
                    st.set(reg::STATUS::C, overflow);
                    st.set(reg::STATUS::DC, Self::dc(a, b));
                    self.w = res;
                });
            }
            LiteralOriented(L { op: XorLiteralWithW, k }) => {
                gen!(@lit {
                    self.w ^= k;
                    self.register.special().status_mut().set(reg::STATUS::Z, self.w == 0);
                });
            }
            LiteralOriented(L { op: OrLiteralWithW, k }) => {
                gen!(@lit {
                    self.w |= k;
                    self.register.special().status_mut().set(reg::STATUS::Z, self.w == 0);
                });
            }
            LiteralOriented(L { op: MoveLiteralToW, k }) => {
                gen!(@lit self.w = k);
            }
            LiteralOriented(L { op: AddLiteralToW, k }) => {
                gen!(@lit self.w += k);
            }
            LiteralOriented(L { op: AndLiteralWithW, k }) => {
                gen!(@lit self.w &= k);
            }
            LiteralOriented(L { op: ReturnWithLiteralInW, k }) => {
                self.w = k;
                self.exec(Instruction::Control(Return));
            }
            Control(i @ (ClearWatchDogTimer | ReturnFromInterrupt | Sleep)) => {
                panic!("unimplemented instruction: {i:?}");
            }
            Control(ClearF { f }) => {
                self.register.at(f).write(0);
                self.register
                    .special()
                    .status_mut()
                    .set(reg::STATUS::Z, true);
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            }
            Control(ClearW) => {
                self.w = 0;
                self.register
                    .special()
                    .status_mut()
                    .set(reg::STATUS::Z, true);
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            }
            Control(MoveWtoF { f }) => {
                self.register.at(f).write(self.w);
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            }
            Control(Goto { addr }) => {
                self.pc = addr.0 * 2;
                self.pc |= ((self.register.special.pclath().read() & 0b0001_1000) as u16) << 8;
                self.ticker.tick(&self.register, 2);
            }
            Control(Call { addr }) => {
                // read: datasheets[0] P25
                self.call_stack
                    .try_push(self.pc + 2)
                    .expect("callstack overflow");
                // pclath: 0b0001_1xxx_0000_0000
                // pc:     0b0000_0111_1111_1111
                self.pc = addr.0 * 2;
                self.pc |= ((self.register.special.pclath().read() & 0b0001_1000) as u16) << 8;
                self.ticker.tick(&self.register, 2);
            }
            Control(Return) => {
                self.pc = self
                    .call_stack
                    .pop()
                    .expect("callstack underflow: callstack have no return address");
                self.ticker.tick(&self.register, 2);
            }
            Control(Noop) => {
                self.pc += 2;
                self.ticker.tick(&self.register, 1);
            }
        }
    }
}

pub mod reg {
    #![allow(dead_code)]

    use crate::inst::RegisterFileAddr;
    use concat_idents::concat_idents;

    pub trait Register {
        fn read(&self) -> u8;
        fn write(&mut self, v: u8);

        // using dyn to preserve object-safety
        fn write_with(&mut self, f: &dyn Fn(u8) -> u8) {
            self.write(f(self.read()))
        }
    }

    pub struct Registers {
        pub special: SpecialPurposeRegisters,
        pub gpr: [GeneralPurposeRegister; 368],
    }

    pub struct GeneralPurposeRegister(pub u8);

    special_registers! {
        // name    field   gen_struct   impl   init        unimpl      unstable on reset
        IADDR      iaddr       y        unimpl 0b0000_0000 0b0000_0000 0b0000_0000
        UNIMPL     unimpl      y        unimpl 0b0000_0000 0b0000_0000 0b0000_0000
        RESERV     reserv      y        unimpl 0b0000_0000 0b0000_0000 0b0000_0000
        TMR0       tmr0        y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        PCL        pcl         y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        STATUS     status      n        none   0b0001_1000 0b0000_0000 0b0000_0111
        FSR        fsr         y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        PORTA      porta       y        stub   0b0000_0000 0b0000_0000 0b1110_0000
        PORTB      portb       y        stub   0b0000_0000 0b0000_0000 0b0011_1111
        PCLATH     pclath      y        stub   0b0000_0000 0b1110_0000 0b0000_0000
        INTCON     intcon      y        stub   0b0000_0000 0b0000_0000 0b0000_0001
        PIR1       pir1        y        stub   0b0000_0000 0b1000_0000 0b0000_0000
        PIR2       pir2        y        stub   0b0000_0000 0b0010_1111 0b0000_0000
        TMR1L      tmr1l       y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        TMR1H      tmr1h       y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        T1CON      t1con       y        stub   0b0000_0000 0b1000_0000 0b0000_0000
        TMR2       tmr2        y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        T2CON      t2con       y        stub   0b0000_0000 0b1000_0000 0b0000_0000
        SSPBUF     sspbuf      y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        SSPCON     sspcon      y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        CCPR1L     ccpr1l      y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        CCPR1H     ccpr1h      y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        CCP1CON    ccp1con     y        stub   0b0000_0000 0b1100_0000 0b0000_0000
        RCSTA      rcsta       y        stub   0b0000_0000 0b0000_0000 0b0000_0001
        TXREG      txreg       y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        RCREG      rcreg       y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        ADRESH     adresh      y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        ADCON0     adcon0      y        stub   0b0000_0000 0b0000_0010 0b0000_0000
        OPTION_REG option_reg  y        stub   0b1111_1111 0b0000_0000 0b0000_0000
        TRISA      trisa       y        stub   0b1111_1111 0b0000_0000 0b0000_0000
        TRISB      trisb       y        stub   0b1111_1111 0b0000_0000 0b0000_0000
        PIE1       pie1        y        stub   0b0000_0000 0b1000_0000 0b0000_0000
        PIE2       pie2        y        stub   0b0000_0000 0b0010_1111 0b0000_0000
        PCON       pcon        y        stub   0b0000_0000 0b1111_1100 0b0000_0000 // NOTE: 0b0000_0001 depends on condition
        OSCCON     osccon      y        stub   0b0000_0000 0b1000_0000 0b0000_0000
        OSCTUNE    osctune     y        stub   0b0000_0000 0b1100_0000 0b0000_0000
        PR2        pr2         y        stub   0b1111_1111 0b0000_0000 0b0000_0000
        SSPADD     sspadd      y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        SSPSTAT    sspstat     y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        TXSTA      txsta       y        stub   0b0000_0010 0b0000_1000 0b0000_0000
        SPBRG      spbrg       y        stub   0b0000_0000 0b0000_0000 0b0000_0000
        ANSEL      ansel       y        stub   0b0111_1111 0b1000_0000 0b0000_0000
        CMCON      cmcon       y        stub   0b0000_0111 0b0000_0000 0b0000_0000
        CVRCON     cvrcon      y        stub   0b0000_0000 0b0001_0000 0b0000_0000
        WDTCON     wdtcon      y        stub   0b0000_1000 0b1110_0000 0b0000_0000
        ADRESL     adresl      y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        ADCON1     adcon1      y        stub   0b0000_0000 0b0000_1111 0b0000_0000
        EEDATA     eedata      y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        EEADR      eeadr       y        stub   0b0000_0000 0b0000_0000 0b1111_1111
        EEDATH     eedath      y        stub   0b0000_0000 0b1100_0000 0b0011_1111
        EEADRH     eeadrh      y        stub   0b0000_0000 0b1111_1000 0b0000_0111
        EECON1     eecon1      y        stub   0b0000_0000 0b0110_0000 0b1001_1000
        EECON2     eecon2      y        stub   0b0000_0000 0b1111_1111 0b0000_0000
    }

    register_map! {
        // bank 0      1          2        3
        0x00 iaddr   iaddr      iaddr    iaddr
        0x01 tmr0    option_reg tmr0     option_reg
        0x02 pcl     pcl        pcl      pcl
        0x03 status  status     status   status
        0x04 fsr     fsr        fsr      fsr
        0x05 porta   trisa      wdtcon   unimpl
        0x06 portb   trisb      portb    trisb
        0x07 unimpl  unimpl     unimpl   unimpl
        0x08 unimpl  unimpl     unimpl   unimpl
        0x09 unimpl  unimpl     unimpl   unimpl
        0x0A pclath  pclath     pclath   pclath
        0x0B intcon  intcon     intcon   intcon
        0x0C pir1    pie1       eedata   eecon1
        0x0D pir2    pie2       eeadr    eecon2
        0x0E tmr1l   pcon       eedath   reserv
        0x0F tmr1h   osccon     eeadrh   reserv
        0x10 t1con   osctune    gpr[176] gpr[272]
        0x11 tmr2    unimpl     gpr[177] gpr[273]
        0x12 t2con   pr2        gpr[178] gpr[274]
        0x13 sspbuf  sspadd     gpr[179] gpr[275]
        0x14 sspcon  sspstat    gpr[180] gpr[276]
        0x15 ccpr1l  unimpl     gpr[181] gpr[277]
        0x16 ccpr1h  unimpl     gpr[182] gpr[278]
        0x17 ccp1con unimpl     gpr[183] gpr[279]
        0x18 rcsta   txsta      gpr[184] gpr[280]
        0x19 txreg   spbrg      gpr[185] gpr[281]
        0x1A rcreg   unimpl     gpr[186] gpr[282]
        0x1B unimpl  unimpl     gpr[187] gpr[283]
        0x1C unimpl  cmcon      gpr[188] gpr[284]
        0x1D unimpl  cvrcon     gpr[189] gpr[285]
        0x1E unimpl  unimpl     gpr[190] gpr[286]
        0x1F unimpl  unimpl     gpr[191] gpr[287]
        0x20 gpr[0]  gpr[96]    gpr[192] gpr[288]
        0x21 gpr[1]  gpr[97]    gpr[193] gpr[289]
        0x22 gpr[2]  gpr[98]    gpr[194] gpr[290]
        0x23 gpr[3]  gpr[99]    gpr[195] gpr[291]
        0x24 gpr[4]  gpr[100]   gpr[196] gpr[292]
        0x25 gpr[5]  gpr[101]   gpr[197] gpr[293]
        0x26 gpr[6]  gpr[102]   gpr[198] gpr[294]
        0x27 gpr[7]  gpr[103]   gpr[199] gpr[295]
        0x28 gpr[8]  gpr[104]   gpr[200] gpr[296]
        0x29 gpr[9]  gpr[105]   gpr[201] gpr[297]
        0x2A gpr[10] gpr[106]   gpr[202] gpr[298]
        0x2B gpr[11] gpr[107]   gpr[203] gpr[299]
        0x2C gpr[12] gpr[108]   gpr[204] gpr[300]
        0x2D gpr[13] gpr[109]   gpr[205] gpr[301]
        0x2E gpr[14] gpr[110]   gpr[206] gpr[302]
        0x2F gpr[15] gpr[111]   gpr[207] gpr[303]
        0x30 gpr[16] gpr[112]   gpr[208] gpr[304]
        0x31 gpr[17] gpr[113]   gpr[209] gpr[305]
        0x32 gpr[18] gpr[114]   gpr[210] gpr[306]
        0x33 gpr[19] gpr[115]   gpr[211] gpr[307]
        0x34 gpr[20] gpr[116]   gpr[212] gpr[308]
        0x35 gpr[21] gpr[117]   gpr[213] gpr[309]
        0x36 gpr[22] gpr[118]   gpr[214] gpr[310]
        0x37 gpr[23] gpr[119]   gpr[215] gpr[311]
        0x38 gpr[24] gpr[120]   gpr[216] gpr[312]
        0x39 gpr[25] gpr[121]   gpr[217] gpr[313]
        0x3A gpr[26] gpr[122]   gpr[218] gpr[314]
        0x3B gpr[27] gpr[123]   gpr[219] gpr[315]
        0x3C gpr[28] gpr[124]   gpr[220] gpr[316]
        0x3D gpr[29] gpr[125]   gpr[221] gpr[317]
        0x3E gpr[30] gpr[126]   gpr[222] gpr[318]
        0x3F gpr[31] gpr[127]   gpr[223] gpr[319]
        0x40 gpr[32] gpr[128]   gpr[224] gpr[320]
        0x41 gpr[33] gpr[129]   gpr[225] gpr[321]
        0x42 gpr[34] gpr[130]   gpr[226] gpr[322]
        0x43 gpr[35] gpr[131]   gpr[227] gpr[323]
        0x44 gpr[36] gpr[132]   gpr[228] gpr[324]
        0x45 gpr[37] gpr[133]   gpr[229] gpr[325]
        0x46 gpr[38] gpr[134]   gpr[230] gpr[326]
        0x47 gpr[39] gpr[135]   gpr[231] gpr[327]
        0x48 gpr[40] gpr[136]   gpr[232] gpr[328]
        0x49 gpr[41] gpr[137]   gpr[233] gpr[329]
        0x4A gpr[42] gpr[138]   gpr[234] gpr[330]
        0x4B gpr[43] gpr[139]   gpr[235] gpr[331]
        0x4C gpr[44] gpr[140]   gpr[236] gpr[332]
        0x4D gpr[45] gpr[141]   gpr[237] gpr[333]
        0x4E gpr[46] gpr[142]   gpr[238] gpr[334]
        0x4F gpr[47] gpr[143]   gpr[239] gpr[335]
        0x50 gpr[48] gpr[144]   gpr[240] gpr[336]
        0x51 gpr[49] gpr[145]   gpr[241] gpr[337]
        0x52 gpr[50] gpr[146]   gpr[242] gpr[338]
        0x53 gpr[51] gpr[147]   gpr[243] gpr[339]
        0x54 gpr[52] gpr[148]   gpr[244] gpr[340]
        0x55 gpr[53] gpr[149]   gpr[245] gpr[341]
        0x56 gpr[54] gpr[150]   gpr[246] gpr[342]
        0x57 gpr[55] gpr[151]   gpr[247] gpr[343]
        0x58 gpr[56] gpr[152]   gpr[248] gpr[344]
        0x59 gpr[57] gpr[153]   gpr[249] gpr[345]
        0x5A gpr[58] gpr[154]   gpr[250] gpr[346]
        0x5B gpr[59] gpr[155]   gpr[251] gpr[347]
        0x5C gpr[60] gpr[156]   gpr[252] gpr[348]
        0x5D gpr[61] gpr[157]   gpr[253] gpr[349]
        0x5E gpr[62] gpr[158]   gpr[254] gpr[350]
        0x5F gpr[63] gpr[159]   gpr[255] gpr[351]
        0x60 gpr[64] gpr[160]   gpr[256] gpr[352]
        0x61 gpr[65] gpr[161]   gpr[257] gpr[353]
        0x62 gpr[66] gpr[162]   gpr[258] gpr[354]
        0x63 gpr[67] gpr[163]   gpr[259] gpr[355]
        0x64 gpr[68] gpr[164]   gpr[260] gpr[356]
        0x65 gpr[69] gpr[165]   gpr[261] gpr[357]
        0x66 gpr[70] gpr[166]   gpr[262] gpr[358]
        0x67 gpr[71] gpr[167]   gpr[263] gpr[359]
        0x68 gpr[72] gpr[168]   gpr[264] gpr[360]
        0x69 gpr[73] gpr[169]   gpr[265] gpr[361]
        0x6A gpr[74] gpr[170]   gpr[266] gpr[362]
        0x6B gpr[75] gpr[171]   gpr[267] gpr[363]
        0x6C gpr[76] gpr[172]   gpr[268] gpr[364]
        0x6D gpr[77] gpr[173]   gpr[269] gpr[365]
        0x6E gpr[78] gpr[174]   gpr[270] gpr[366]
        0x6F gpr[79] gpr[175]   gpr[271] gpr[367]
        0x70 gpr[80] gpr[80]    gpr[80]  gpr[80]  // `accesses`
        0x71 gpr[81] gpr[81]    gpr[81]  gpr[81]
        0x72 gpr[82] gpr[82]    gpr[82]  gpr[82]
        0x73 gpr[83] gpr[83]    gpr[83]  gpr[83]
        0x74 gpr[84] gpr[84]    gpr[84]  gpr[84]
        0x75 gpr[85] gpr[85]    gpr[85]  gpr[85]
        0x76 gpr[86] gpr[86]    gpr[86]  gpr[86]
        0x77 gpr[87] gpr[87]    gpr[87]  gpr[87]
        0x78 gpr[88] gpr[88]    gpr[88]  gpr[88]
        0x79 gpr[89] gpr[89]    gpr[89]  gpr[89]
        0x7A gpr[90] gpr[90]    gpr[90]  gpr[90]
        0x7B gpr[91] gpr[91]    gpr[91]  gpr[91]
        0x7C gpr[92] gpr[92]    gpr[92]  gpr[92]
        0x7D gpr[93] gpr[93]    gpr[93]  gpr[93]
        0x7E gpr[94] gpr[94]    gpr[94]  gpr[94]
        0x7F gpr[95] gpr[95]    gpr[95]  gpr[95]
    }

    impl Default for Registers {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Registers {
        pub fn new() -> Self {
            Self {
                special: SpecialPurposeRegisters::new(),
                gpr: std::array::from_fn(|_| GeneralPurposeRegister::new()),
            }
        }

        pub fn special(&mut self) -> &mut SpecialPurposeRegisters {
            &mut self.special
        }
    }

    impl Default for GeneralPurposeRegister {
        fn default() -> Self {
            Self::new()
        }
    }

    impl GeneralPurposeRegister {
        pub fn new() -> Self {
            Self(0)
        }
    }

    impl Register for GeneralPurposeRegister {
        fn read(&self) -> u8 {
            // TODO: check for uninitilized?
            self.0
        }

        fn write(&mut self, v: u8) {
            self.0 = v;
        }
    }

    macro_rules! register_map {
        ($($addr:literal $bank0:ident$([$index0:literal])? $bank1:ident$([$index1:literal])? $bank2:ident$([$index2:literal])? $bank3:ident$([$index3:literal])?)+) => {
            impl Registers {
                pub fn at(&mut self, addr: RegisterFileAddr) -> &mut dyn Register {
                    let bank = (self.special.status_mut().read() & 0b0110_0000) >> 5;
                    match (bank, addr.0) {
                        (4.., _) => panic!("bank out of bounds"),
                        (_, 0x80..) => panic!("addr out of bounds"),
                        $(
                            (0, $addr) => &mut register_map!(@outexpr self $bank0$([$index0])?),
                            (1, $addr) => &mut register_map!(@outexpr self $bank1$([$index1])?),
                            (2, $addr) => &mut register_map!(@outexpr self $bank2$([$index2])?),
                            (3, $addr) => &mut register_map!(@outexpr self $bank3$([$index3])?),
                        )+
                    }
                }
            }
        };

        (@outexpr $me:ident gpr[$index:literal]) => {
            $me.gpr[$index]
        };

        (@outexpr $me:ident $name:ident) => {
            $me.special.$name
        };
    }

    macro_rules! special_registers {
        (
            $($name:ident $lowername:ident $gen_struct:ident $stub_ty:ident $initial_value:literal $unimplemented_mask:literal $unknown_mask:literal)+
        ) => {
            $(
                special_registers!(@struct $name $gen_struct $unimplemented_mask $initial_value);
                special_registers!(@genstub $name $stub_ty);

                impl Default for $name {
                    fn default() -> Self {
                        Self::new()
                    }
                }

            )+

            pub struct SpecialPurposeRegisters {
                $($lowername: $name,)+
            }

            impl Default for SpecialPurposeRegisters {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl SpecialPurposeRegisters {
                pub fn new() -> Self {
                    Self {
                        $($lowername: $name::new(),)+
                    }
                }

                $(
                    concat_idents! { mut_fn_name = $lowername, _mut {
                        pub fn mut_fn_name(&mut self) -> &mut $name {
                            &mut self.$lowername
                        }
                        pub fn $lowername(&self) -> &$name {
                            &self.$lowername
                        }
                    }}
                )+
            }
        };

        (@struct $name:ident y $unimplemented_mask:literal $initial_value:literal) => {
            #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
            pub struct $name(pub u8);

            impl $name {
                const UNIMPLEMENTED: u8 = $unimplemented_mask;
                pub fn new() -> Self {
                    Self($initial_value)
                }
            }
        };

        (@struct $name:ident n $unimplemented_mask:literal $initial_value:literal) => {
            impl $name {
                const UNIMPLEMENTED: u8 = $unimplemented_mask;
                const INITIAL_VALUE: u8 = $initial_value;
            }
        };

        (@genstub stub) => { };

        (@genstub $name:ident stub) => {
            impl Register for $name {
                fn read(&self) -> u8 {
                    // log::warn!("{}: read stub!", stringify!($name));
                    self.0
                }

                fn write(&mut self, v: u8) {
                    // log::warn!("{}: write stub!", stringify!($name));
                    self.0 = v;
                }
            }
        };

        (@genstub $name:ident none) => { };

        (@genstub $name:ident unimpl) => {
            impl Register for $name {
                fn read(&self) -> u8 {
                    log::warn!("{}: tried to read the reserved register!: reading 0", stringify!($name));
                    0
                }

                fn write(&mut self, _v: u8) {
                    panic!("{}: attempted to write on the reserved register", stringify!($name));
                }
            }
        };
    }

    bitflags::bitflags! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct STATUS: u8 {
            const IRP = 1 << 7;
            const RP1 = 1 << 6;
            const RP0 = 1 << 5;
            const TO  = 1 << 4;
            const PD  = 1 << 3;
            const Z   = 1 << 2;
            const DC  = 1 << 1;
            const C   = 1 << 0;
        }
    }

    impl STATUS {
        fn new() -> Self {
            Self::from_bits(Self::INITIAL_VALUE).unwrap()
        }
    }
    impl Register for STATUS {
        fn read(&self) -> u8 {
            self.bits()
        }

        fn write(&mut self, v: u8) {
            *self = Self::from_bits(v).unwrap();
        }
    }

    use register_map;
    use special_registers;
}
