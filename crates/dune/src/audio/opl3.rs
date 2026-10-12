//! OPL2/OPL3 FM synthesis: a Rust port of Nuked OPL3 (Nuke.YKT, LGPL 2.1).
//!
//! The C version wires slots and channels together with pointers; here those become
//! `Src` values that name where a modulator input or channel output comes from.

const LOGSINROM: [u16; 256] = [
    0x859, 0x6c3, 0x607, 0x58b, 0x52e, 0x4e4, 0x4a6, 0x471, 0x443, 0x41a, 0x3f5, 0x3d3, 0x3b5, 0x398, 0x37e, 0x365, 0x34e, 0x339,
    0x324, 0x311, 0x2ff, 0x2ed, 0x2dc, 0x2cd, 0x2bd, 0x2af, 0x2a0, 0x293, 0x286, 0x279, 0x26d, 0x261, 0x256, 0x24b, 0x240, 0x236,
    0x22c, 0x222, 0x218, 0x20f, 0x206, 0x1fd, 0x1f5, 0x1ec, 0x1e4, 0x1dc, 0x1d4, 0x1cd, 0x1c5, 0x1be, 0x1b7, 0x1b0, 0x1a9, 0x1a2,
    0x19b, 0x195, 0x18f, 0x188, 0x182, 0x17c, 0x177, 0x171, 0x16b, 0x166, 0x160, 0x15b, 0x155, 0x150, 0x14b, 0x146, 0x141, 0x13c,
    0x137, 0x133, 0x12e, 0x129, 0x125, 0x121, 0x11c, 0x118, 0x114, 0x10f, 0x10b, 0x107, 0x103, 0x0ff, 0x0fb, 0x0f8, 0x0f4, 0x0f0,
    0x0ec, 0x0e9, 0x0e5, 0x0e2, 0x0de, 0x0db, 0x0d7, 0x0d4, 0x0d1, 0x0cd, 0x0ca, 0x0c7, 0x0c4, 0x0c1, 0x0be, 0x0bb, 0x0b8, 0x0b5,
    0x0b2, 0x0af, 0x0ac, 0x0a9, 0x0a7, 0x0a4, 0x0a1, 0x09f, 0x09c, 0x099, 0x097, 0x094, 0x092, 0x08f, 0x08d, 0x08a, 0x088, 0x086,
    0x083, 0x081, 0x07f, 0x07d, 0x07a, 0x078, 0x076, 0x074, 0x072, 0x070, 0x06e, 0x06c, 0x06a, 0x068, 0x066, 0x064, 0x062, 0x060,
    0x05e, 0x05c, 0x05b, 0x059, 0x057, 0x055, 0x053, 0x052, 0x050, 0x04e, 0x04d, 0x04b, 0x04a, 0x048, 0x046, 0x045, 0x043, 0x042,
    0x040, 0x03f, 0x03e, 0x03c, 0x03b, 0x039, 0x038, 0x037, 0x035, 0x034, 0x033, 0x031, 0x030, 0x02f, 0x02e, 0x02d, 0x02b, 0x02a,
    0x029, 0x028, 0x027, 0x026, 0x025, 0x024, 0x023, 0x022, 0x021, 0x020, 0x01f, 0x01e, 0x01d, 0x01c, 0x01b, 0x01a, 0x019, 0x018,
    0x017, 0x017, 0x016, 0x015, 0x014, 0x014, 0x013, 0x012, 0x011, 0x011, 0x010, 0x00f, 0x00f, 0x00e, 0x00d, 0x00d, 0x00c, 0x00c,
    0x00b, 0x00a, 0x00a, 0x009, 0x009, 0x008, 0x008, 0x007, 0x007, 0x007, 0x006, 0x006, 0x005, 0x005, 0x005, 0x004, 0x004, 0x004,
    0x003, 0x003, 0x003, 0x002, 0x002, 0x002, 0x002, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x000, 0x000, 0x000, 0x000,
    0x000, 0x000, 0x000, 0x000,
];

const EXPROM: [u16; 256] = [
    0x7fa, 0x7f5, 0x7ef, 0x7ea, 0x7e4, 0x7df, 0x7da, 0x7d4, 0x7cf, 0x7c9, 0x7c4, 0x7bf, 0x7b9, 0x7b4, 0x7ae, 0x7a9, 0x7a4, 0x79f,
    0x799, 0x794, 0x78f, 0x78a, 0x784, 0x77f, 0x77a, 0x775, 0x770, 0x76a, 0x765, 0x760, 0x75b, 0x756, 0x751, 0x74c, 0x747, 0x742,
    0x73d, 0x738, 0x733, 0x72e, 0x729, 0x724, 0x71f, 0x71a, 0x715, 0x710, 0x70b, 0x706, 0x702, 0x6fd, 0x6f8, 0x6f3, 0x6ee, 0x6e9,
    0x6e5, 0x6e0, 0x6db, 0x6d6, 0x6d2, 0x6cd, 0x6c8, 0x6c4, 0x6bf, 0x6ba, 0x6b5, 0x6b1, 0x6ac, 0x6a8, 0x6a3, 0x69e, 0x69a, 0x695,
    0x691, 0x68c, 0x688, 0x683, 0x67f, 0x67a, 0x676, 0x671, 0x66d, 0x668, 0x664, 0x65f, 0x65b, 0x657, 0x652, 0x64e, 0x649, 0x645,
    0x641, 0x63c, 0x638, 0x634, 0x630, 0x62b, 0x627, 0x623, 0x61e, 0x61a, 0x616, 0x612, 0x60e, 0x609, 0x605, 0x601, 0x5fd, 0x5f9,
    0x5f5, 0x5f0, 0x5ec, 0x5e8, 0x5e4, 0x5e0, 0x5dc, 0x5d8, 0x5d4, 0x5d0, 0x5cc, 0x5c8, 0x5c4, 0x5c0, 0x5bc, 0x5b8, 0x5b4, 0x5b0,
    0x5ac, 0x5a8, 0x5a4, 0x5a0, 0x59c, 0x599, 0x595, 0x591, 0x58d, 0x589, 0x585, 0x581, 0x57e, 0x57a, 0x576, 0x572, 0x56f, 0x56b,
    0x567, 0x563, 0x560, 0x55c, 0x558, 0x554, 0x551, 0x54d, 0x549, 0x546, 0x542, 0x53e, 0x53b, 0x537, 0x534, 0x530, 0x52c, 0x529,
    0x525, 0x522, 0x51e, 0x51b, 0x517, 0x514, 0x510, 0x50c, 0x509, 0x506, 0x502, 0x4ff, 0x4fb, 0x4f8, 0x4f4, 0x4f1, 0x4ed, 0x4ea,
    0x4e7, 0x4e3, 0x4e0, 0x4dc, 0x4d9, 0x4d6, 0x4d2, 0x4cf, 0x4cc, 0x4c8, 0x4c5, 0x4c2, 0x4be, 0x4bb, 0x4b8, 0x4b5, 0x4b1, 0x4ae,
    0x4ab, 0x4a8, 0x4a4, 0x4a1, 0x49e, 0x49b, 0x498, 0x494, 0x491, 0x48e, 0x48b, 0x488, 0x485, 0x482, 0x47e, 0x47b, 0x478, 0x475,
    0x472, 0x46f, 0x46c, 0x469, 0x466, 0x463, 0x460, 0x45d, 0x45a, 0x457, 0x454, 0x451, 0x44e, 0x44b, 0x448, 0x445, 0x442, 0x43f,
    0x43c, 0x439, 0x436, 0x433, 0x430, 0x42d, 0x42a, 0x428, 0x425, 0x422, 0x41f, 0x41c, 0x419, 0x416, 0x414, 0x411, 0x40e, 0x40b,
    0x408, 0x406, 0x403, 0x400,
];

const MT: [u32; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];
const KSLROM: [u8; 16] = [0, 32, 40, 45, 48, 51, 53, 55, 56, 58, 59, 60, 61, 62, 63, 64];
const KSLSHIFT: [u8; 4] = [8, 1, 2, 0];
const EG_INCSTEP: [[u8; 4]; 4] = [[0, 0, 0, 0], [1, 0, 0, 0], [1, 0, 1, 0], [1, 1, 1, 0]];
const AD_SLOT: [i8; 0x20] = [0, 1, 2, 3, 4, 5, -1, -1, 6, 7, 8, 9, 10, 11, -1, -1, 12, 13, 14, 15, 16, 17, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1];
const CH_SLOT: [usize; 18] = [0, 1, 2, 6, 7, 8, 12, 13, 14, 18, 19, 20, 24, 25, 26, 30, 31, 32];

const CH_2OP: u8 = 0;
const CH_4OP: u8 = 1;
const CH_4OP2: u8 = 2;
const CH_DRUM: u8 = 3;
const EGK_NORM: u8 = 1;
const EGK_DRUM: u8 = 2;
const EG_ATTACK: u8 = 0;
const EG_DECAY: u8 = 1;
const EG_SUSTAIN: u8 = 2;
const EG_RELEASE: u8 = 3;

/// Where a modulation input / channel output reads from (the C version's int16_t pointers).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Src {
    Zero,
    Out(usize),
    Fbmod(usize),
}

#[derive(Clone, Copy, Default)]
struct Slot {
    channel: usize,
    out: i16,
    fbmod: i16,
    mod_: Src,
    prout: i16,
    eg_rout: u16,
    eg_out: u16,
    eg_gen: u8,
    eg_ksl: u8,
    trem: bool,
    reg_vib: u8,
    reg_type: u8,
    reg_ksr: u8,
    reg_mult: u8,
    reg_ksl: u8,
    reg_tl: u8,
    reg_ar: u8,
    reg_dr: u8,
    reg_sl: u8,
    reg_rr: u8,
    reg_wf: u8,
    key: u8,
    pg_reset: u32,
    pg_phase: u32,
    pg_phase_out: u16,
    slot_num: u8,
}

impl Default for Src {
    fn default() -> Self {
        Src::Zero
    }
}

#[derive(Clone, Copy, Default)]
struct Channel {
    slots: [usize; 2],
    pair: usize,
    out: [Src; 4],
    chtype: u8,
    f_num: u16,
    f_num_reg: u16,
    block: u8,
    block_reg: u8,
    fb: u8,
    con: u8,
    alg: u8,
    ksv: u8,
    cha: u16,
    chb: u16,
    chc: u16,
    chd: u16,
    ch_num: u8,
}

pub struct Opl3 {
    channel: [Channel; 18],
    slot: [Slot; 36],
    timer: u16,
    eg_timer: u64,
    eg_timerrem: u8,
    eg_state: u8,
    eg_add: u8,
    eg_timer_lo: u8,
    newm: u8,
    nts: u8,
    rhy: u8,
    vibpos: u8,
    vibshift: u8,
    tremolo: u8,
    tremolopos: u8,
    tremoloshift: u8,
    noise: u32,
    mixbuff: [i32; 4],
    rm_hh_bit2: u8,
    rm_hh_bit3: u8,
    rm_hh_bit7: u8,
    rm_hh_bit8: u8,
    rm_tc_bit3: u8,
    rm_tc_bit5: u8,
    rateratio: i32,
    samplecnt: i32,
    oldsamples: [i16; 4],
    samples: [i16; 4],
}

fn envelope_calc_exp(level: u32) -> i16 {
    let level = level.min(0x1fff);
    (((EXPROM[(level & 0xff) as usize] as u32) << 1) >> (level >> 8)) as i16
}

fn envelope_sin(wf: u8, phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let env = (envelope as u32) << 3;
    let ls = |i: u16| LOGSINROM[(i & 0xff) as usize] as u32;
    match wf {
        0 => {
            let neg: u16 = if phase & 0x200 != 0 { 0xffff } else { 0 };
            let out = if phase & 0x100 != 0 { ls((phase & 0xff) ^ 0xff) } else { ls(phase) };
            (envelope_calc_exp(out + env) as u16 ^ neg) as i16
        }
        1 => {
            let out = if phase & 0x200 != 0 {
                0x1000
            } else if phase & 0x100 != 0 {
                ls((phase & 0xff) ^ 0xff)
            } else {
                ls(phase)
            };
            envelope_calc_exp(out + env)
        }
        2 => {
            let out = if phase & 0x100 != 0 { ls((phase & 0xff) ^ 0xff) } else { ls(phase) };
            envelope_calc_exp(out + env)
        }
        3 => {
            let out = if phase & 0x100 != 0 { 0x1000 } else { ls(phase) };
            envelope_calc_exp(out + env)
        }
        4 => {
            let neg: u16 = if phase & 0x300 == 0x100 { 0xffff } else { 0 };
            let out = if phase & 0x200 != 0 {
                0x1000
            } else if phase & 0x80 != 0 {
                ls(((phase ^ 0xff) << 1) & 0xff)
            } else {
                ls((phase << 1) & 0xff)
            };
            (envelope_calc_exp(out + env) as u16 ^ neg) as i16
        }
        5 => {
            let out = if phase & 0x200 != 0 {
                0x1000
            } else if phase & 0x80 != 0 {
                ls(((phase ^ 0xff) << 1) & 0xff)
            } else {
                ls((phase << 1) & 0xff)
            };
            envelope_calc_exp(out + env)
        }
        6 => {
            let neg: u16 = if phase & 0x200 != 0 { 0xffff } else { 0 };
            (envelope_calc_exp(env) as u16 ^ neg) as i16
        }
        _ => {
            let mut phase = phase;
            let mut neg: u16 = 0;
            if phase & 0x200 != 0 {
                neg = 0xffff;
                phase = (phase & 0x1ff) ^ 0x1ff;
            }
            let out = (phase as u32) << 3;
            (envelope_calc_exp(out + env) as u16 ^ neg) as i16
        }
    }
}

impl Opl3 {
    /// OPL3_Reset
    pub fn new(samplerate: u32) -> Opl3 {
        let mut c = Opl3 {
            channel: [Channel::default(); 18],
            slot: [Slot::default(); 36],
            timer: 0,
            eg_timer: 0,
            eg_timerrem: 0,
            eg_state: 0,
            eg_add: 0,
            eg_timer_lo: 0,
            newm: 0,
            nts: 0,
            rhy: 0,
            vibpos: 0,
            vibshift: 1,
            tremolo: 0,
            tremolopos: 0,
            tremoloshift: 4,
            noise: 1,
            mixbuff: [0; 4],
            rm_hh_bit2: 0,
            rm_hh_bit3: 0,
            rm_hh_bit7: 0,
            rm_hh_bit8: 0,
            rm_tc_bit3: 0,
            rm_tc_bit5: 0,
            rateratio: ((samplerate as i64) << 10) as i32 / 49716,
            samplecnt: 0,
            oldsamples: [0; 4],
            samples: [0; 4],
        };
        for (i, s) in c.slot.iter_mut().enumerate() {
            s.mod_ = Src::Zero;
            s.eg_rout = 0x1ff;
            s.eg_out = 0x1ff;
            s.eg_gen = EG_RELEASE;
            s.trem = false;
            s.slot_num = i as u8;
        }
        for ch in 0..18 {
            let ls = CH_SLOT[ch];
            c.channel[ch].slots = [ls, ls + 3];
            c.slot[ls].channel = ch;
            c.slot[ls + 3].channel = ch;
            c.channel[ch].pair = if ch % 9 < 3 {
                ch + 3
            } else if ch % 9 < 6 {
                ch - 3
            } else {
                ch
            };
            c.channel[ch].out = [Src::Zero; 4];
            c.channel[ch].chtype = CH_2OP;
            c.channel[ch].cha = 0xffff;
            c.channel[ch].chb = 0xffff;
            c.channel[ch].ch_num = ch as u8;
            c.channel_setup_alg(ch);
        }
        c
    }

    fn src(&self, s: Src) -> i16 {
        match s {
            Src::Zero => 0,
            Src::Out(i) => self.slot[i].out,
            Src::Fbmod(i) => self.slot[i].fbmod,
        }
    }

    fn envelope_update_ksl(&mut self, s: usize) {
        let ch = &self.channel[self.slot[s].channel];
        let ksl = ((KSLROM[(ch.f_num >> 6) as usize] as i16) << 2) - ((0x08 - ch.block as i16) << 5);
        self.slot[s].eg_ksl = ksl.max(0) as u8;
    }

    fn envelope_calc(&mut self, s: usize) {
        let chip_eg_add = self.eg_add;
        let chip_eg_state = self.eg_state;
        let chip_eg_timer_lo = self.eg_timer_lo;
        let trem = if self.slot[s].trem { self.tremolo } else { 0 };
        let ksv = self.channel[self.slot[s].channel].ksv;
        let slot = &mut self.slot[s];
        slot.eg_out = slot.eg_rout + ((slot.reg_tl as u16) << 2) + ((slot.eg_ksl as u16) >> KSLSHIFT[slot.reg_ksl as usize]) + trem as u16;
        let mut reset = 0u32;
        let mut reg_rate = 0u8;
        if slot.key != 0 && slot.eg_gen == EG_RELEASE {
            reset = 1;
            reg_rate = slot.reg_ar;
        } else {
            match slot.eg_gen {
                EG_ATTACK => reg_rate = slot.reg_ar,
                EG_DECAY => reg_rate = slot.reg_dr,
                EG_SUSTAIN => {
                    if slot.reg_type == 0 {
                        reg_rate = slot.reg_rr;
                    }
                }
                _ => reg_rate = slot.reg_rr,
            }
        }
        slot.pg_reset = reset;
        let ks = ksv >> ((slot.reg_ksr ^ 1) << 1);
        let nonzero = reg_rate != 0;
        let rate = ks + (reg_rate << 2);
        let mut rate_hi = rate >> 2;
        let rate_lo = rate & 0x03;
        if rate_hi & 0x10 != 0 {
            rate_hi = 0x0f;
        }
        let eg_shift = rate_hi + chip_eg_add;
        let mut shift: u8 = 0;
        if nonzero {
            if rate_hi < 12 {
                if chip_eg_state != 0 {
                    match eg_shift {
                        12 => shift = 1,
                        13 => shift = (rate_lo >> 1) & 0x01,
                        14 => shift = rate_lo & 0x01,
                        _ => {}
                    }
                }
            } else {
                shift = (rate_hi & 0x03) + EG_INCSTEP[rate_lo as usize][chip_eg_timer_lo as usize];
                if shift & 0x04 != 0 {
                    shift = 0x03;
                }
                if shift == 0 {
                    shift = chip_eg_state;
                }
            }
        }
        let mut eg_rout = slot.eg_rout;
        let mut eg_inc: i16 = 0;
        let mut eg_off = false;
        if reset != 0 && rate_hi == 0x0f {
            eg_rout = 0;
        }
        if slot.eg_rout & 0x1f8 == 0x1f8 {
            eg_off = true;
        }
        if slot.eg_gen != EG_ATTACK && reset == 0 && eg_off {
            eg_rout = 0x1ff;
        }
        match slot.eg_gen {
            EG_ATTACK => {
                if slot.eg_rout == 0 {
                    slot.eg_gen = EG_DECAY;
                } else if slot.key != 0 && shift > 0 && rate_hi != 0x0f {
                    eg_inc = (!(slot.eg_rout as i16)) >> (4 - shift);
                }
            }
            EG_DECAY => {
                if (slot.eg_rout >> 4) as u8 == slot.reg_sl {
                    slot.eg_gen = EG_SUSTAIN;
                } else if !eg_off && reset == 0 && shift > 0 {
                    eg_inc = 1 << (shift - 1);
                }
            }
            _ => {
                if !eg_off && reset == 0 && shift > 0 {
                    eg_inc = 1 << (shift - 1);
                }
            }
        }
        slot.eg_rout = ((eg_rout as i16).wrapping_add(eg_inc) as u16) & 0x1ff;
        if reset != 0 {
            slot.eg_gen = EG_ATTACK;
        }
        if slot.key == 0 {
            slot.eg_gen = EG_RELEASE;
        }
    }

    fn phase_generate(&mut self, s: usize) {
        let ch = self.slot[s].channel;
        let mut f_num = self.channel[ch].f_num;
        if self.slot[s].reg_vib != 0 {
            let mut range = ((f_num >> 7) & 7) as i8;
            let vibpos = self.vibpos;
            if vibpos & 3 == 0 {
                range = 0;
            } else if vibpos & 1 != 0 {
                range >>= 1;
            }
            range >>= self.vibshift;
            if vibpos & 4 != 0 {
                range = -range;
            }
            f_num = (f_num as i16 + range as i16) as u16;
        }
        let basefreq = ((f_num as u32) << self.channel[ch].block) >> 1;
        let slot = &mut self.slot[s];
        let phase = (slot.pg_phase >> 9) as u16;
        if slot.pg_reset != 0 {
            slot.pg_phase = 0;
        }
        slot.pg_phase = slot.pg_phase.wrapping_add((basefreq * MT[slot.reg_mult as usize]) >> 1);
        let noise = self.noise;
        slot.pg_phase_out = phase;
        let slot_num = slot.slot_num;
        if slot_num == 13 {
            self.rm_hh_bit2 = ((phase >> 2) & 1) as u8;
            self.rm_hh_bit3 = ((phase >> 3) & 1) as u8;
            self.rm_hh_bit7 = ((phase >> 7) & 1) as u8;
            self.rm_hh_bit8 = ((phase >> 8) & 1) as u8;
        }
        if slot_num == 17 && self.rhy & 0x20 != 0 {
            self.rm_tc_bit3 = ((phase >> 3) & 1) as u8;
            self.rm_tc_bit5 = ((phase >> 5) & 1) as u8;
        }
        if self.rhy & 0x20 != 0 {
            let rm_xor = (self.rm_hh_bit2 ^ self.rm_hh_bit7) | (self.rm_hh_bit3 ^ self.rm_tc_bit5) | (self.rm_tc_bit3 ^ self.rm_tc_bit5);
            let slot = &mut self.slot[s];
            match slot_num {
                13 => {
                    slot.pg_phase_out = (rm_xor as u16) << 9;
                    if (rm_xor as u32 ^ (noise & 1)) != 0 {
                        slot.pg_phase_out |= 0xd0;
                    } else {
                        slot.pg_phase_out |= 0x34;
                    }
                }
                16 => {
                    slot.pg_phase_out = ((self.rm_hh_bit8 as u16) << 9) | (((self.rm_hh_bit8 as u32 ^ (noise & 1)) as u16) << 8);
                }
                17 => slot.pg_phase_out = ((rm_xor as u16) << 9) | 0x80,
                _ => {}
            }
        }
        let n_bit = ((noise >> 14) ^ noise) & 0x01;
        self.noise = (noise >> 1) | (n_bit << 22);
    }

    fn slot_write_20(&mut self, s: usize, data: u8) {
        let slot = &mut self.slot[s];
        slot.trem = (data >> 7) & 1 != 0;
        slot.reg_vib = (data >> 6) & 1;
        slot.reg_type = (data >> 5) & 1;
        slot.reg_ksr = (data >> 4) & 1;
        slot.reg_mult = data & 0x0f;
    }

    fn slot_write_40(&mut self, s: usize, data: u8) {
        self.slot[s].reg_ksl = (data >> 6) & 3;
        self.slot[s].reg_tl = data & 0x3f;
        self.envelope_update_ksl(s);
    }

    fn slot_write_60(&mut self, s: usize, data: u8) {
        self.slot[s].reg_ar = (data >> 4) & 0x0f;
        self.slot[s].reg_dr = data & 0x0f;
    }

    fn slot_write_80(&mut self, s: usize, data: u8) {
        let slot = &mut self.slot[s];
        slot.reg_sl = (data >> 4) & 0x0f;
        if slot.reg_sl == 0x0f {
            slot.reg_sl = 0x1f;
        }
        slot.reg_rr = data & 0x0f;
    }

    fn slot_write_e0(&mut self, s: usize, data: u8) {
        self.slot[s].reg_wf = data & 0x07;
        if self.newm == 0 {
            self.slot[s].reg_wf &= 0x03;
        }
    }

    fn slot_generate(&mut self, s: usize) {
        let m = self.src(self.slot[s].mod_);
        let slot = &mut self.slot[s];
        slot.out = envelope_sin(slot.reg_wf, slot.pg_phase_out.wrapping_add(m as u16), slot.eg_out);
    }

    fn slot_calc_fb(&mut self, s: usize) {
        let fb = self.channel[self.slot[s].channel].fb;
        let slot = &mut self.slot[s];
        if fb != 0 {
            slot.fbmod = ((slot.prout as i32 + slot.out as i32) >> (0x09 - fb)) as i16;
        } else {
            slot.fbmod = 0;
        }
        slot.prout = slot.out;
    }

    fn envelope_key_on(&mut self, s: usize, t: u8) {
        self.slot[s].key |= t;
    }

    fn envelope_key_off(&mut self, s: usize, t: u8) {
        self.slot[s].key &= !t;
    }

    fn channel_update_rhythm(&mut self, data: u8) {
        self.rhy = data & 0x3f;
        if self.rhy & 0x20 != 0 {
            let s6 = self.channel[6].slots;
            let s7 = self.channel[7].slots;
            let s8 = self.channel[8].slots;
            self.channel[6].out = [Src::Out(s6[1]), Src::Out(s6[1]), Src::Zero, Src::Zero];
            self.channel[7].out = [Src::Out(s7[0]), Src::Out(s7[0]), Src::Out(s7[1]), Src::Out(s7[1])];
            self.channel[8].out = [Src::Out(s8[0]), Src::Out(s8[0]), Src::Out(s8[1]), Src::Out(s8[1])];
            for ch in 6..9 {
                self.channel[ch].chtype = CH_DRUM;
            }
            self.channel_setup_alg(6);
            self.channel_setup_alg(7);
            self.channel_setup_alg(8);
            let set = |c: &mut Opl3, bit: u8, s: usize| {
                if c.rhy & bit != 0 {
                    c.envelope_key_on(s, EGK_DRUM)
                } else {
                    c.envelope_key_off(s, EGK_DRUM)
                }
            };
            set(self, 0x01, s7[0]);
            set(self, 0x02, s8[1]);
            set(self, 0x04, s8[0]);
            set(self, 0x08, s7[1]);
            set(self, 0x10, s6[0]);
            set(self, 0x10, s6[1]);
        } else {
            for ch in 6..9 {
                self.channel[ch].chtype = CH_2OP;
                self.channel_setup_alg(ch);
                let s = self.channel[ch].slots;
                self.envelope_key_off(s[0], EGK_DRUM);
                self.envelope_key_off(s[1], EGK_DRUM);
            }
        }
    }

    fn channel_update_frequency(&mut self, ch: usize) {
        let c = &self.channel[ch];
        self.channel[ch].ksv = (c.block << 1) | ((c.f_num >> (0x09 - self.nts)) & 0x01) as u8;
        let s = self.channel[ch].slots;
        self.envelope_update_ksl(s[0]);
        self.envelope_update_ksl(s[1]);
    }

    fn channel_restore_frequency(&mut self, ch: usize) {
        self.channel[ch].f_num = self.channel[ch].f_num_reg;
        self.channel[ch].block = self.channel[ch].block_reg;
        self.channel_update_frequency(ch);
    }

    fn channel_sync_4op(&mut self, ch: usize) {
        let p = self.channel[ch].pair;
        self.channel[p].f_num = self.channel[ch].f_num;
        self.channel[p].block = self.channel[ch].block;
        self.channel_update_frequency(p);
    }

    fn channel_write_a0(&mut self, ch: usize, data: u8) {
        self.channel[ch].f_num_reg = (self.channel[ch].f_num_reg & 0x300) | data as u16;
        if self.channel[ch].chtype == CH_4OP2 {
            return;
        }
        self.channel_restore_frequency(ch);
        if self.channel[ch].chtype == CH_4OP {
            self.channel_sync_4op(ch);
        }
    }

    fn channel_write_b0(&mut self, ch: usize, data: u8) {
        self.channel[ch].f_num_reg = (self.channel[ch].f_num_reg & 0xff) | (((data & 0x03) as u16) << 8);
        self.channel[ch].block_reg = (data >> 2) & 0x07;
        if self.channel[ch].chtype == CH_4OP2 {
            return;
        }
        self.channel_restore_frequency(ch);
        if self.channel[ch].chtype == CH_4OP {
            self.channel_sync_4op(ch);
        }
    }

    fn channel_setup_alg(&mut self, ch: usize) {
        let c = self.channel[ch];
        let [s0, s1] = c.slots;
        if c.chtype == CH_DRUM {
            if c.ch_num == 7 || c.ch_num == 8 {
                self.slot[s0].mod_ = Src::Zero;
                self.slot[s1].mod_ = Src::Zero;
                return;
            }
            match c.alg & 0x01 {
                0 => {
                    self.slot[s0].mod_ = Src::Fbmod(s0);
                    self.slot[s1].mod_ = Src::Out(s0);
                }
                _ => {
                    self.slot[s0].mod_ = Src::Fbmod(s0);
                    self.slot[s1].mod_ = Src::Zero;
                }
            }
            return;
        }
        if c.alg & 0x08 != 0 {
            return;
        }
        if c.alg & 0x04 != 0 {
            let p = c.pair;
            let [p0, p1] = self.channel[p].slots;
            self.channel[p].out = [Src::Zero; 4];
            match c.alg & 0x03 {
                0 => {
                    self.slot[p0].mod_ = Src::Fbmod(p0);
                    self.slot[p1].mod_ = Src::Out(p0);
                    self.slot[s0].mod_ = Src::Out(p1);
                    self.slot[s1].mod_ = Src::Out(s0);
                    self.channel[ch].out = [Src::Out(s1), Src::Zero, Src::Zero, Src::Zero];
                }
                1 => {
                    self.slot[p0].mod_ = Src::Fbmod(p0);
                    self.slot[p1].mod_ = Src::Out(p0);
                    self.slot[s0].mod_ = Src::Zero;
                    self.slot[s1].mod_ = Src::Out(s0);
                    self.channel[ch].out = [Src::Out(p1), Src::Out(s1), Src::Zero, Src::Zero];
                }
                2 => {
                    self.slot[p0].mod_ = Src::Fbmod(p0);
                    self.slot[p1].mod_ = Src::Zero;
                    self.slot[s0].mod_ = Src::Out(p1);
                    self.slot[s1].mod_ = Src::Out(s0);
                    self.channel[ch].out = [Src::Out(p0), Src::Out(s1), Src::Zero, Src::Zero];
                }
                _ => {
                    self.slot[p0].mod_ = Src::Fbmod(p0);
                    self.slot[p1].mod_ = Src::Zero;
                    self.slot[s0].mod_ = Src::Out(p1);
                    self.slot[s1].mod_ = Src::Zero;
                    self.channel[ch].out = [Src::Out(p0), Src::Out(s0), Src::Out(s1), Src::Zero];
                }
            }
        } else {
            match c.alg & 0x01 {
                0 => {
                    self.slot[s0].mod_ = Src::Fbmod(s0);
                    self.slot[s1].mod_ = Src::Out(s0);
                    self.channel[ch].out = [Src::Out(s1), Src::Zero, Src::Zero, Src::Zero];
                }
                _ => {
                    self.slot[s0].mod_ = Src::Fbmod(s0);
                    self.slot[s1].mod_ = Src::Zero;
                    self.channel[ch].out = [Src::Out(s0), Src::Out(s1), Src::Zero, Src::Zero];
                }
            }
        }
    }

    fn channel_update_alg(&mut self, ch: usize) {
        let c = self.channel[ch];
        self.channel[ch].alg = c.con;
        if c.chtype == CH_4OP {
            let p = c.pair;
            self.channel[p].alg = 0x04 | (c.con << 1) | self.channel[p].con;
            self.channel[ch].alg = 0x08;
            self.channel_setup_alg(p);
        } else if c.chtype == CH_4OP2 {
            let p = c.pair;
            self.channel[ch].alg = 0x04 | (self.channel[p].con << 1) | c.con;
            self.channel[p].alg = 0x08;
            self.channel_setup_alg(ch);
        } else {
            self.channel_setup_alg(ch);
        }
    }

    fn channel_write_c0(&mut self, ch: usize, data: u8) {
        self.channel[ch].fb = (data & 0x0e) >> 1;
        self.channel[ch].con = data & 0x01;
        self.channel_update_alg(ch);
        let c = &mut self.channel[ch];
        if self.newm != 0 {
            c.cha = if (data >> 4) & 1 != 0 { 0xffff } else { 0 };
            c.chb = if (data >> 5) & 1 != 0 { 0xffff } else { 0 };
            c.chc = if (data >> 6) & 1 != 0 { 0xffff } else { 0 };
            c.chd = if (data >> 7) & 1 != 0 { 0xffff } else { 0 };
        } else {
            c.cha = 0xffff;
            c.chb = 0xffff;
            c.chc = 0;
            c.chd = 0;
        }
    }

    fn channel_key_on(&mut self, ch: usize) {
        let c = self.channel[ch];
        if c.chtype == CH_4OP {
            let p = self.channel[c.pair].slots;
            self.envelope_key_on(c.slots[0], EGK_NORM);
            self.envelope_key_on(c.slots[1], EGK_NORM);
            self.envelope_key_on(p[0], EGK_NORM);
            self.envelope_key_on(p[1], EGK_NORM);
        } else if c.chtype == CH_2OP || c.chtype == CH_DRUM {
            self.envelope_key_on(c.slots[0], EGK_NORM);
            self.envelope_key_on(c.slots[1], EGK_NORM);
        }
    }

    fn channel_key_off(&mut self, ch: usize) {
        let c = self.channel[ch];
        if c.chtype == CH_4OP {
            let p = self.channel[c.pair].slots;
            self.envelope_key_off(c.slots[0], EGK_NORM);
            self.envelope_key_off(c.slots[1], EGK_NORM);
            self.envelope_key_off(p[0], EGK_NORM);
            self.envelope_key_off(p[1], EGK_NORM);
        } else if c.chtype == CH_2OP || c.chtype == CH_DRUM {
            self.envelope_key_off(c.slots[0], EGK_NORM);
            self.envelope_key_off(c.slots[1], EGK_NORM);
        }
    }

    fn channel_set_4op(&mut self, data: u8) {
        for bit in 0..6u8 {
            let mut chnum = bit as usize;
            if bit >= 3 {
                chnum += 9 - 3;
            }
            if (data >> bit) & 1 != 0 {
                self.channel[chnum].chtype = CH_4OP;
                self.channel[chnum + 3].chtype = CH_4OP2;
                self.channel_sync_4op(chnum);
                self.channel_update_alg(chnum);
            } else {
                self.channel[chnum].chtype = CH_2OP;
                self.channel[chnum + 3].chtype = CH_2OP;
                self.channel_restore_frequency(chnum + 3);
                self.channel_update_alg(chnum);
                self.channel_update_alg(chnum + 3);
            }
        }
    }

    fn process_slot(&mut self, s: usize) {
        self.slot_calc_fb(s);
        self.envelope_calc(s);
        self.phase_generate(s);
        self.slot_generate(s);
    }

    fn mix(&self, left: bool) -> (i32, i32) {
        let mut m = (0i32, 0i32);
        for c in &self.channel {
            let accm = self.src(c.out[0]).wrapping_add(self.src(c.out[1])).wrapping_add(self.src(c.out[2])).wrapping_add(self.src(c.out[3]));
            if left {
                m.0 += (accm as u16 & c.cha) as i16 as i32;
                m.1 += (accm as u16 & c.chc) as i16 as i32;
            } else {
                m.0 += (accm as u16 & c.chb) as i16 as i32;
                m.1 += (accm as u16 & c.chd) as i16 as i32;
            }
        }
        m
    }

    /// OPL3_Generate4Ch (native 49716 Hz)
    fn generate_4ch(&mut self, buf4: &mut [i16; 4]) {
        let clip = |v: i32| v.clamp(-32768, 32767) as i16;
        buf4[1] = clip(self.mixbuff[1]);
        buf4[3] = clip(self.mixbuff[3]);
        for i in 0..15 {
            self.process_slot(i);
        }
        let (a, b) = self.mix(true);
        self.mixbuff[0] = a;
        self.mixbuff[2] = b;
        for i in 15..18 {
            self.process_slot(i);
        }
        buf4[0] = clip(self.mixbuff[0]);
        buf4[2] = clip(self.mixbuff[2]);
        for i in 18..33 {
            self.process_slot(i);
        }
        let (a, b) = self.mix(false);
        self.mixbuff[1] = a;
        self.mixbuff[3] = b;
        for i in 33..36 {
            self.process_slot(i);
        }
        if self.timer & 0x3f == 0x3f {
            self.tremolopos = (self.tremolopos + 1) % 210;
        }
        self.tremolo = if self.tremolopos < 105 { self.tremolopos >> self.tremoloshift } else { (210 - self.tremolopos) >> self.tremoloshift };
        if self.timer & 0x3ff == 0x3ff {
            self.vibpos = (self.vibpos + 1) & 7;
        }
        self.timer = self.timer.wrapping_add(1);
        if self.eg_state != 0 {
            let mut shift = 0u8;
            while shift < 13 && (self.eg_timer >> shift) & 1 == 0 {
                shift += 1;
            }
            self.eg_add = if shift > 12 { 0 } else { shift + 1 };
            self.eg_timer_lo = (self.eg_timer & 3) as u8;
        }
        if self.eg_timerrem != 0 || self.eg_state != 0 {
            if self.eg_timer == 0xfffffffff {
                self.eg_timer = 0;
                self.eg_timerrem = 1;
            } else {
                self.eg_timer += 1;
                self.eg_timerrem = 0;
            }
        }
        self.eg_state ^= 1;
    }

    /// OPL3_GenerateResampled: one sample at the configured output rate (left channel).
    pub fn generate_resampled(&mut self) -> i16 {
        while self.samplecnt >= self.rateratio {
            self.oldsamples = self.samples;
            let mut s = [0i16; 4];
            self.generate_4ch(&mut s);
            self.samples = s;
            self.samplecnt -= self.rateratio;
        }
        let out = (self.oldsamples[0] as i32 * (self.rateratio - self.samplecnt) + self.samples[0] as i32 * self.samplecnt) / self.rateratio;
        self.samplecnt += 1 << 10;
        out as i16
    }

    /// OPL3_WriteReg
    pub fn write_reg(&mut self, reg: u16, v: u8) {
        let high = ((reg >> 8) & 1) as usize;
        let regm = (reg & 0xff) as u8;
        let slot_idx = |r: u8| -> Option<usize> {
            let a = AD_SLOT[(r & 0x1f) as usize];
            (a >= 0).then(|| 18 * high + a as usize)
        };
        match regm & 0xf0 {
            0x00 => {
                if high != 0 {
                    match regm & 0x0f {
                        0x04 => self.channel_set_4op(v),
                        0x05 => self.newm = v & 1,
                        _ => {}
                    }
                } else if regm & 0x0f == 0x08 {
                    self.nts = (v >> 6) & 1;
                }
            }
            0x20 | 0x30 => {
                if let Some(s) = slot_idx(regm) {
                    self.slot_write_20(s, v)
                }
            }
            0x40 | 0x50 => {
                if let Some(s) = slot_idx(regm) {
                    self.slot_write_40(s, v)
                }
            }
            0x60 | 0x70 => {
                if let Some(s) = slot_idx(regm) {
                    self.slot_write_60(s, v)
                }
            }
            0x80 | 0x90 => {
                if let Some(s) = slot_idx(regm) {
                    self.slot_write_80(s, v)
                }
            }
            0xe0 | 0xf0 => {
                if let Some(s) = slot_idx(regm) {
                    self.slot_write_e0(s, v)
                }
            }
            0xa0 => {
                if regm & 0x0f < 9 {
                    self.channel_write_a0(9 * high + (regm & 0x0f) as usize, v);
                }
            }
            0xb0 => {
                if regm == 0xbd && high == 0 {
                    self.tremoloshift = (((v >> 7) ^ 1) << 1) + 2;
                    self.vibshift = ((v >> 6) & 1) ^ 1;
                    self.channel_update_rhythm(v);
                } else if regm & 0x0f < 9 {
                    let ch = 9 * high + (regm & 0x0f) as usize;
                    self.channel_write_b0(ch, v);
                    if v & 0x20 != 0 {
                        self.channel_key_on(ch);
                    } else {
                        self.channel_key_off(ch);
                    }
                }
            }
            0xc0 => {
                if regm & 0x0f < 9 {
                    self.channel_write_c0(9 * high + (regm & 0x0f) as usize, v);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_after_reset() {
        let mut c = Opl3::new(48000);
        let v: Vec<i16> = (0..1000).map(|_| c.generate_resampled()).collect();
        assert!(v.iter().all(|&s| s == 0), "{:?}", &v[..20]);
    }
}
