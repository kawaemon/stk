# INDF N/A N/A N/A
data = """TMR0 xxxx xxxx uuuu uuuu uuuu uuuu
PCL 0000 0000 PC + 1(2)
STATUS 0001 1xxx 000q quuu(3) uuuq quuu(3)
FSR xxxx xxxx uuuu uuuu uuuu uuuu
PORTA xxx0 0000 uuu0 0000 uuuu uuuu
PORTB 00xx xxxx 00uu uuuu uuuu uuuu
PCLATH ---0 0000 ---0 0000 ---u uuuu
INTCON 0000 000x 0000 000u uuuu uuuu(1)
PIR1 -000 0000 -000 0000 -uuu uuuu(1)
PIR2 00-0 ---- 00-0 ---- uu-u ----(1)
TMR1L xxxx xxxx uuuu uuuu uuuu uuuu
TMR1H xxxx xxxx uuuu uuuu uuuu uuuu
T1CON -000 0000 -uuu uuuu -uuu uuuu
TMR2 0000 0000 0000 0000 uuuu uuuu
T2CON -000 0000 -000 0000 -uuu uuuu
SSPBUF xxxx xxxx uuuu uuuu uuuu uuuu
SSPCON 0000 0000 0000 0000 uuuu uuuu
CCPR1L xxxx xxxx uuuu uuuu uuuu uuuu
CCPR1H xxxx xxxx uuuu uuuu uuuu uuuu
CCP1CON --00 0000 --00 0000 --uu uuuu
RCSTA 0000 000x 0000 000x uuuu uuuu
TXREG 0000 0000 0000 0000 uuuu uuuu
RCREG 0000 0000 0000 0000 uuuu uuuu
ADRESH xxxx xxxx uuuu uuuu uuuu uuuu
ADCON0 0000 00-0 0000 00-0 uuuu uu-u
OPTION_REG 1111 1111 1111 1111 uuuu uuuu
TRISA 1111 1111 1111 1111 uuuu uuuu
TRISB 1111 1111 1111 1111 uuuu uuuu
PIE1 -000 0000 -000 0000 -uuu uuuu
PIE2 00-0 ---- 00-0 ---- uu-u ----
PCON ---- --0q ---- --uu ---- --uu
OSCCON -000 0000 -000 0000 -uuu uuuu
OSCTUNE --00 0000 --00 0000 --uu uuuu
PR2 1111 1111 1111 1111 1111 1111
SSPADD 0000 0000 0000 0000 uuuu uuuu
SSPSTAT 0000 0000 0000 0000 uuuu uuuu
TXSTA 0000 -010 0000 -010 uuuu -u1u
SPBRG 0000 0000 0000 0000 uuuu uuuu
ANSEL -111 1111 -111 1111 -111 1111
CMCON 0000 0111 0000 0111 uuuu u111
CVRCON 000- 0000 000- 0000 uuu- uuuu
WDTCON ---0 1000 ---0 1000 ---u uuuu
ADRESL xxxx xxxx uuuu uuuu uuuu uuuu
ADCON1 0000 ---- 0000 ---- uuuu ----
EEDATA xxxx xxxx uuuu uuuu uuuu uuuu
EEADR xxxx xxxx uuuu uuuu uuuu uuuu
EEDATH --xx xxxx --uu uuuu --uu uuuu
EEADRH ---- -xxx ---- -uuu ---- -uuu
EECON1 x--x x000 u--x u000 u--u uuuu
EECON2 ---- ---- ---- ---- ---- ----"""

addrs = """0x001 0x002 0x003 0x004 0x005
0x006 0x00A 0x00B 0x00C 0x00D
0x00E 0x00F 0x010 0x011 0x012
0x013 0x014 0x015 0x016 0x017
0x018 0x019 0x01A 0x01E 0x01F
0x081 0x085 0x086 0x08C 0x08D
0x08E 0x08F 0x090 0x092 0x093
0x094 0x098 0x099 0x09B 0x09C
0x09D 0x09E 0x09F 0x105 0x10C
0x10D 0x10E 0x10F 0x18C 0x18D""".replace("\n", " ")

# u = unchanged, x = unknown, - = unimplemented bit, read as ‘0’, q = value depends on condition

UNKNOWN_BIT_REPR = "x"
UNIMPLEMENTED_BIT_REPR = "-"
DEPENDS_ON_CONDITION_REPR = "q"

BINARY_LITERAL_HEADER = "0b"
INITIAL_BIT_FOR_UNIMPLEMENTED = "0"
INITIAL_BIT_FOR_UNKNOWN = "0"
INITIAL_BIT_FOR_DEPENDS_ON_CONDITION = "0"

def _4bit_align(s):
    left = len(BINARY_LITERAL_HEADER) + 4
    return s[:left] + "_" + s[left:]

def masks(raw):
    unimplemented = BINARY_LITERAL_HEADER
    unknown = BINARY_LITERAL_HEADER
    initial = BINARY_LITERAL_HEADER
    depends_on_condition = BINARY_LITERAL_HEADER
    for c in raw:
        if c == UNKNOWN_BIT_REPR:
            unimplemented += "0"
            unknown += "1"
            depends_on_condition += "0"
            initial += INITIAL_BIT_FOR_UNKNOWN
        elif c == UNIMPLEMENTED_BIT_REPR:
            unimplemented += "1"
            unknown += "0"
            depends_on_condition += "0"
            initial += INITIAL_BIT_FOR_UNIMPLEMENTED
        elif c == DEPENDS_ON_CONDITION_REPR:
            unimplemented += "0"
            unknown += "0"
            initial += INITIAL_BIT_FOR_DEPENDS_ON_CONDITION
            depends_on_condition += "1"
        elif c == "0" or c == "1":
            unimplemented += "0"
            unknown += "0"
            initial += c
            depends_on_condition += "0"
        else:
            raise Exception(f"unknown repr: {c}")

    res = " ".join([_4bit_align(initial), _4bit_align(unimplemented), _4bit_align(unknown)])
    if depends_on_condition != "0b00000000":
        res += f" // NOTE: {_4bit_align(depends_on_condition)} depends on condition"

    return res


print("# masks")
for line, addr in zip(data.split("\n"), addrs.split(" ")):
    segments = line.split(" ")
    print(f"{segments[0]:10s} {addr} {masks(segments[1] + segments[2])}")

