use arrayvec::ArrayVec;

use crate::{
    inst::{
        BitOrientedInstruction, BitOrientedOperation, ByteOrientedInstruction,
        ByteOrientedOperation, ControlInstruction, Destination, Instruction,
        LiteralOrientedInstruction, LiteralOrientedOperation, RegisterFileAddr,
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
    fn tick(&mut self, clocks: u8);
}

impl<T: Ticker> P16F88<T> {
    #[allow(clippy::new_without_default)]
    pub fn new(ticker: T) -> Self {
        P16F88 {
            w: 0,
            pc: 0,
            flash: [0; 7168],
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
                self.ticker.tick(1);
            };

            (@byte $f:ident, $d:ident, |$r:ident| $op:expr) => {
                match $d {
                    Destination::W => {
                        let $r = self.register.at($f).read();
                        self.w = $op;
                    }
                    Destination::F => {
                        self.register.at($f).write_with(&|$r| $op);
                    }
                }
                self.pc += 2;
                self.ticker.tick(1);
            };
        }

        // TODO: overflow check
        // TODO: status flags

        match inst {
            #[rustfmt::skip]
            ByteOriented(Y { op: AddWf, f, dest }) => {
                gen!(@byte f, dest, |x| self.w + x);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: AndWf, f, dest }) => {
                gen!(@byte f, dest, |x| self.w & x);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: ComplementF, f, dest }) => {
                // read: datasheets[1] P20
                gen!(@byte f, dest, |x| !x);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: DecrementF, f, dest }) => {
                gen!(@byte f, dest, |x| x - 1);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: IncrementF, f, dest }) => {
                gen!(@byte f, dest, |x| x + 1);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: DecrementFSkipIfZ, f, dest }) => {
                let res = self.register.at(f).read() - 1;
                match dest {
                    Destination::W => self.w = res,
                    Destination::F => self.register.at(f).write(res),
                }
                let skip = res == 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(if skip { 2 } else { 1 });
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: IncrementFSkipIfZ, f, dest }) => {
                let res = self.register.at(f).read() + 1;
                match dest {
                    Destination::W => self.w = res,
                    Destination::F => self.register.at(f).write(res),
                }
                let skip = res == 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(if skip { 2 } else { 1 });
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: OrWf, f, dest }) => {
                gen!(@byte f, dest, |x| self.w | x);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: MoveF, f, dest }) => {
                gen!(@byte f, dest, |x| x);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: RotateLeftFThroughCarry, f, dest }) => {
                todo!()
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: RotateRightFThroughCarry, f, dest }) => {
                todo!()
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: SubtractWfromF, f, dest }) => {
                gen!(@byte f, dest, |x| x - self.w);
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: SwapF, f, dest }) => {
                gen!(@byte f, dest, |x| {
                    let left = (x & 0b1111_0000) >> 4;
                    let right = x & 0b0000_1111;
                    (right << 4) | left
                });
            }
            #[rustfmt::skip]
            ByteOriented(Y { op: XorWwithF, f, dest }) => {
                gen!(@byte f, dest, |x| self.w ^ x);
            }
            #[rustfmt::skip]
            BitOriented(B { op: BitClearF, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                self.register.at(f).write_with(&|x| x & (!mask));
                self.pc += 2;
                self.ticker.tick(1);
            }
            #[rustfmt::skip]
            BitOriented(B { op: BitSetF, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                self.register.at(f).write_with(&|x| x | mask);
                self.pc += 2;
                self.ticker.tick(1);
            }
            #[rustfmt::skip]
            BitOriented(B { op: SkipIfFBitClear, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                let skip = (self.register.at(f).read() & mask) == 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(if skip { 2 } else { 1 });
            }
            #[rustfmt::skip]
            BitOriented(B { op: SkipIfFBitSet, b, f }) => {
                let mask = 0b0000_0001 << b.0;
                let skip = (self.register.at(f).read() & mask) != 0;
                self.pc += if skip { 4 } else { 2 };
                self.ticker.tick(if skip { 2 } else { 1 });
            }
            #[rustfmt::skip]
            LiteralOriented(L { op: SubtractWFromLiteral, k }) => {
                gen!(@lit self.w = k - self.w);
            }
            #[rustfmt::skip]
            LiteralOriented(L { op: XorLiteralWithW, k }) => {
                gen!(@lit self.w ^= k);
            }
            #[rustfmt::skip]
            LiteralOriented(L { op: OrLiteralWithW, k }) => {
                gen!(@lit self.w |= k);
            }
            #[rustfmt::skip]
            LiteralOriented(L { op: MoveLiteralToW, k }) => {
                gen!(@lit self.w = k);
            }
            #[rustfmt::skip]
            LiteralOriented(L { op: AddLiteralToW, k }) => {
                gen!(@lit self.w += k);
            }
            #[rustfmt::skip]
            LiteralOriented(L { op: AndLiteralWithW, k }) => {
                gen!(@lit self.w &= k);
            }
            #[rustfmt::skip]
            LiteralOriented(L { op: ReturnWithLiteralInW, k }) => {
                self.w = k;
                self.exec(Instruction::Control(Return));
            }
            Control(i @ (ClearWatchDogTimer | ReturnFromInterrupt | Sleep)) => {
                panic!("unimplemented instruction: {i:?}");
            }
            Control(ClearF { f }) => {
                self.register.at(f).write(0);
                self.pc += 2;
                self.ticker.tick(1);
            }
            Control(ClearW) => {
                self.w = 0;
                self.pc += 2;
                self.ticker.tick(1);
            }
            Control(MoveWtoF { f }) => {
                self.register.at(f).write(self.w);
                self.pc += 2;
                self.ticker.tick(1);
            }
            Control(Goto { addr }) => {
                self.pc = addr.0;
                self.pc |= ((self.register.special.pclath().read() & 0b0001_1000) as u16) << 8;
                self.ticker.tick(1);
            }
            Control(Call { addr }) => {
                // read: datasheets[0] P25
                self.call_stack
                    .try_push(self.pc + 2)
                    .expect("callstack overflow");
                // pclath: 0b0001_1xxx_0000_0000
                // pc:     0b0000_0111_1111_1111
                self.pc = addr.0;
                self.pc |= ((self.register.special.pclath().read() & 0b0001_1000) as u16) << 8;
                self.ticker.tick(2);
            }
            Control(Return) => {
                self.pc = self
                    .call_stack
                    .pop()
                    .expect("callstack underflow: callstack have no return address");
                self.ticker.tick(2);
            }
            Control(Noop) => {
                self.pc += 2;
                self.ticker.tick(1);
            }
        }
    }
}

pub mod reg {
    #![allow(dead_code)]

    use crate::inst::RegisterFileAddr;

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

    pub struct GeneralPurposeRegister(u8);

    special_registers! {
        // name    field      impl   init        unimpl      unstable on reset
        IADDR      iaddr      unimpl 0b0000_0000 0b0000_0000 0b0000_0000
        UNIMPL     unimpl     unimpl 0b0000_0000 0b0000_0000 0b0000_0000
        RESERV     reserv     unimpl 0b0000_0000 0b0000_0000 0b0000_0000
        TMR0       tmr0       stub   0b0000_0000 0b0000_0000 0b1111_1111
        PCL        pcl        stub   0b0000_0000 0b0000_0000 0b0000_0000
        STATUS     status     stub   0b0001_1000 0b0000_0000 0b0000_0111
        FSR        fsr        stub   0b0000_0000 0b0000_0000 0b1111_1111
        PORTA      porta      stub   0b0000_0000 0b0000_0000 0b1110_0000
        PORTB      portb      stub   0b0000_0000 0b0000_0000 0b0011_1111
        PCLATH     pclath     stub   0b0000_0000 0b1110_0000 0b0000_0000
        INTCON     intcon     stub   0b0000_0000 0b0000_0000 0b0000_0001
        PIR1       pir1       stub   0b0000_0000 0b1000_0000 0b0000_0000
        PIR2       pir2       stub   0b0000_0000 0b0010_1111 0b0000_0000
        TMR1L      tmr1l      stub   0b0000_0000 0b0000_0000 0b1111_1111
        TMR1H      tmr1h      stub   0b0000_0000 0b0000_0000 0b1111_1111
        T1CON      t1con      stub   0b0000_0000 0b1000_0000 0b0000_0000
        TMR2       tmr2       stub   0b0000_0000 0b0000_0000 0b0000_0000
        T2CON      t2con      stub   0b0000_0000 0b1000_0000 0b0000_0000
        SSPBUF     sspbuf     stub   0b0000_0000 0b0000_0000 0b1111_1111
        SSPCON     sspcon     stub   0b0000_0000 0b0000_0000 0b0000_0000
        CCPR1L     ccpr1l     stub   0b0000_0000 0b0000_0000 0b1111_1111
        CCPR1H     ccpr1h     stub   0b0000_0000 0b0000_0000 0b1111_1111
        CCP1CON    ccp1con    stub   0b0000_0000 0b1100_0000 0b0000_0000
        RCSTA      rcsta      stub   0b0000_0000 0b0000_0000 0b0000_0001
        TXREG      txreg      stub   0b0000_0000 0b0000_0000 0b0000_0000
        RCREG      rcreg      stub   0b0000_0000 0b0000_0000 0b0000_0000
        ADRESH     adresh     stub   0b0000_0000 0b0000_0000 0b1111_1111
        ADCON0     adcon0     stub   0b0000_0000 0b0000_0010 0b0000_0000
        OPTION_REG option_reg stub   0b1111_1111 0b0000_0000 0b0000_0000
        TRISA      trisa      stub   0b1111_1111 0b0000_0000 0b0000_0000
        TRISB      trisb      stub   0b1111_1111 0b0000_0000 0b0000_0000
        PIE1       pie1       stub   0b0000_0000 0b1000_0000 0b0000_0000
        PIE2       pie2       stub   0b0000_0000 0b0010_1111 0b0000_0000
        PCON       pcon       stub   0b0000_0000 0b1111_1100 0b0000_0000 // NOTE: 0b0000_0001 depends on condition
        OSCCON     osccon     stub   0b0000_0000 0b1000_0000 0b0000_0000
        OSCTUNE    osctune    stub   0b0000_0000 0b1100_0000 0b0000_0000
        PR2        pr2        stub   0b1111_1111 0b0000_0000 0b0000_0000
        SSPADD     sspadd     stub   0b0000_0000 0b0000_0000 0b0000_0000
        SSPSTAT    sspstat    stub   0b0000_0000 0b0000_0000 0b0000_0000
        TXSTA      txsta      stub   0b0000_0010 0b0000_1000 0b0000_0000
        SPBRG      spbrg      stub   0b0000_0000 0b0000_0000 0b0000_0000
        ANSEL      ansel      stub   0b0111_1111 0b1000_0000 0b0000_0000
        CMCON      cmcon      stub   0b0000_0111 0b0000_0000 0b0000_0000
        CVRCON     cvrcon     stub   0b0000_0000 0b0001_0000 0b0000_0000
        WDTCON     wdtcon     stub   0b0000_1000 0b1110_0000 0b0000_0000
        ADRESL     adresl     stub   0b0000_0000 0b0000_0000 0b1111_1111
        ADCON1     adcon1     stub   0b0000_0000 0b0000_1111 0b0000_0000
        EEDATA     eedata     stub   0b0000_0000 0b0000_0000 0b1111_1111
        EEADR      eeadr      stub   0b0000_0000 0b0000_0000 0b1111_1111
        EEDATH     eedath     stub   0b0000_0000 0b1100_0000 0b0011_1111
        EEADRH     eeadrh     stub   0b0000_0000 0b1111_1000 0b0000_0111
        EECON1     eecon1     stub   0b0000_0000 0b0110_0000 0b1001_1000
        EECON2     eecon2     stub   0b0000_0000 0b1111_1111 0b0000_0000
    }

    register_map! {
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
            // super-dirty hack to initialize 368 gprs
            macro_rules! init {
                (@root $($a:ident),+$(,)?) => { [$($a::new(),)+] };
                (@x16  $($a:ident),+$(,)?) => {
                    init!(@root $(
                        $a, $a, $a, $a,
                        $a, $a, $a, $a,
                        $a, $a, $a, $a,
                        $a, $a, $a, $a,
                    )+)
                };
                (@x16x23 $($a:ident),+$(,)?) => {
                    init!(@x16 $(
                        $a, $a, $a, $a, $a, $a,
                        $a, $a, $a, $a, $a, $a,
                        $a, $a, $a, $a, $a, $a,
                        $a, $a, $a, $a, $a,
                    )+)
                };
            }

            Self {
                special: SpecialPurposeRegisters::new(),
                gpr: init!(@x16x23 GeneralPurposeRegister),
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
                    let bank = (self.special.status().read() & 0b0110_0000) >> 5;
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
            $($name:ident $lowername:ident $($stub_ty:ident)? $initial_value:literal $unimplemented_mask:literal $unknown_mask:literal)+
        ) => {
            $(
                #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
                pub struct $name(u8);

                $(special_registers!(@genstub $name $stub_ty);)?

                impl Default for $name {
                    fn default() -> Self {
                        Self::new()
                    }
                }

                impl $name {
                    const UNIMPLEMENTED: u8 = $unimplemented_mask;
                    pub fn new() -> Self {
                        Self($initial_value)
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
                    pub fn $lowername(&mut self) -> &mut $name {
                        &mut self.$lowername
                    }
                )+
            }
        };

        (@genstub stub) => { };

        (@genstub $name:ident stub) => {
            impl Register for $name {
                fn read(&self) -> u8 {
                    log::warn!("{}: read stub!", stringify!($name));
                    self.0
                }

                fn write(&mut self, v: u8) {
                    log::warn!("{}: write stub!", stringify!($name));
                    self.0 = v;
                }
            }
        };

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

    use register_map;
    use special_registers;
}
