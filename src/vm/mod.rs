use crate::inst::{ControlInstruction, Instruction};

pub struct P16F88 {
    w: u8,
    pc: u16,
    register: [u8; 0x0200],
    flash: [u8; 7168],
}

impl P16F88 {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        struct Visitor {
            register: [u8; 0x0200],
        }
        impl RegisterVisitor for Visitor {
            fn visit(&mut self, reg: impl DynRegisterDef) {
                self.register[reg.addr() as usize] = reg.init();
            }
        }

        let mut v = Visitor {
            register: [0; 0x0200],
        };
        reg::visit_all_registers(&mut v);

        P16F88 {
            w: 0,
            pc: 0,
            register: v.register,
            flash: [0; 7168],
        }
    }

    pub fn exec(&mut self, inst: Instruction) -> Self {
        match inst {
            Instruction::ByteOriented(_) => todo!(),
            Instruction::BitOriented(_) => todo!(),
            Instruction::LiteralOriented(_) => todo!(),
            Instruction::Control(ControlInstruction::Call { addr }) => todo!(),
        }
    }
}

pub trait RegisterDef {
    const NAME: &'static str;
    const INIT: u8;
    const ADDR: u16;
}

pub trait DynRegisterDef {
    fn name(&self) -> &'static str;
    fn init(&self) -> u8;
    fn addr(&self) -> u16;
}

impl<T: RegisterDef> DynRegisterDef for T {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn init(&self) -> u8 {
        T::INIT
    }

    fn addr(&self) -> u16 {
        T::ADDR
    }
}

pub trait RegisterVisitor {
    fn visit(&mut self, reg: impl DynRegisterDef);
}

pub mod reg {
    use super::*;

    macro_rules! registers {
        ($($name:ident $addr:literal $initial_value:literal $unimplemented_mask:literal $unknown_mask:literal)+) => {
            $(
                #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
                pub struct $name;
                impl RegisterDef for $name {
                    const NAME: &'static str = stringify!($name);
                    const INIT: u8 = $initial_value;
                    const ADDR: u16 = $addr;
                }
            )+

            pub fn visit_all_registers(f: &mut impl RegisterVisitor) {
                $(
                    f.visit($name);
                )+
            }
        };
    }

    registers! {
        // name    addr  init        unimplemented unknown
        TMR0       0x001 0b0000_0000 0b0000_0000 0b1111_1111
        PCL        0x002 0b0000_0000 0b0000_0000 0b0000_0000
        STATUS     0x003 0b0001_1000 0b0000_0000 0b0000_0111
        FSR        0x004 0b0000_0000 0b0000_0000 0b1111_1111
        PORTA      0x005 0b0000_0000 0b0000_0000 0b1110_0000
        PORTB      0x006 0b0000_0000 0b0000_0000 0b0011_1111
        PCLATH     0x00A 0b0000_0000 0b1110_0000 0b0000_0000
        INTCON     0x00B 0b0000_0000 0b0000_0000 0b0000_0001
        PIR1       0x00C 0b0000_0000 0b1000_0000 0b0000_0000
        PIR2       0x00D 0b0000_0000 0b0010_1111 0b0000_0000
        TMR1L      0x00E 0b0000_0000 0b0000_0000 0b1111_1111
        TMR1H      0x00F 0b0000_0000 0b0000_0000 0b1111_1111
        T1CON      0x010 0b0000_0000 0b1000_0000 0b0000_0000
        TMR2       0x011 0b0000_0000 0b0000_0000 0b0000_0000
        T2CON      0x012 0b0000_0000 0b1000_0000 0b0000_0000
        SSPBUF     0x013 0b0000_0000 0b0000_0000 0b1111_1111
        SSPCON     0x014 0b0000_0000 0b0000_0000 0b0000_0000
        CCPR1L     0x015 0b0000_0000 0b0000_0000 0b1111_1111
        CCPR1H     0x016 0b0000_0000 0b0000_0000 0b1111_1111
        CCP1CON    0x017 0b0000_0000 0b1100_0000 0b0000_0000
        RCSTA      0x018 0b0000_0000 0b0000_0000 0b0000_0001
        TXREG      0x019 0b0000_0000 0b0000_0000 0b0000_0000
        RCREG      0x01A 0b0000_0000 0b0000_0000 0b0000_0000
        ADRESH     0x01E 0b0000_0000 0b0000_0000 0b1111_1111
        ADCON0     0x01F 0b0000_0000 0b0000_0010 0b0000_0000
        OPTION_REG 0x081 0b1111_1111 0b0000_0000 0b0000_0000
        TRISA      0x085 0b1111_1111 0b0000_0000 0b0000_0000
        TRISB      0x086 0b1111_1111 0b0000_0000 0b0000_0000
        PIE1       0x08C 0b0000_0000 0b1000_0000 0b0000_0000
        PIE2       0x08D 0b0000_0000 0b0010_1111 0b0000_0000
        PCON       0x08E 0b0000_0000 0b1111_1100 0b0000_0000 // NOTE: 0b0000_0001 depends on condition
        OSCCON     0x08F 0b0000_0000 0b1000_0000 0b0000_0000
        OSCTUNE    0x090 0b0000_0000 0b1100_0000 0b0000_0000
        PR2        0x092 0b1111_1111 0b0000_0000 0b0000_0000
        SSPADD     0x093 0b0000_0000 0b0000_0000 0b0000_0000
        SSPSTAT    0x094 0b0000_0000 0b0000_0000 0b0000_0000
        TXSTA      0x098 0b0000_0010 0b0000_1000 0b0000_0000
        SPBRG      0x099 0b0000_0000 0b0000_0000 0b0000_0000
        ANSEL      0x09B 0b0111_1111 0b1000_0000 0b0000_0000
        CMCON      0x09C 0b0000_0111 0b0000_0000 0b0000_0000
        CVRCON     0x09D 0b0000_0000 0b0001_0000 0b0000_0000
        WDTCON     0x09E 0b0000_1000 0b1110_0000 0b0000_0000
        ADRESL     0x09F 0b0000_0000 0b0000_0000 0b1111_1111
        ADCON1     0x105 0b0000_0000 0b0000_1111 0b0000_0000
        EEDATA     0x10C 0b0000_0000 0b0000_0000 0b1111_1111
        EEADR      0x10D 0b0000_0000 0b0000_0000 0b1111_1111
        EEDATH     0x10E 0b0000_0000 0b1100_0000 0b0011_1111
        EEADRH     0x10F 0b0000_0000 0b1111_1000 0b0000_0111
        EECON1     0x18C 0b0000_0000 0b0110_0000 0b1001_1000
        EECON2     0x18D 0b0000_0000 0b1111_1111 0b0000_0000
    }
}
