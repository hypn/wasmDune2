//! The two random number generators used by Dune II (Tools_Random_256 and the Borland LCG).

#[derive(Clone, Debug, Default)]
pub struct Rng {
    seed: [u8; 4],
    lcg: u32,
}

impl Rng {
    pub fn new() -> Self {
        Self::default()
    }

    /// Tools_Random_Seed
    pub fn seed_256(&mut self, seed: u32) {
        self.seed = seed.to_le_bytes();
    }

    /// Tools_Random_256
    pub fn random_256(&mut self) -> u8 {
        let s = &mut self.seed;
        let mut val16: u16 = ((s[1] as u16) << 8) | s[2] as u16;
        let mut val8: u8 = (((val16 ^ 0x8000) >> 15) & 1) as u8;
        val16 = (val16 << 1) | ((s[0] >> 1) & 1) as u16;
        val8 = (s[0] >> 2).wrapping_sub(s[0]).wrapping_sub(val8);
        s[0] = (val8 << 7) | (s[0] >> 1);
        s[1] = (val16 >> 8) as u8;
        s[2] = (val16 & 0xFF) as u8;
        s[0] ^ s[1]
    }

    /// Tools_RandomLCG_Seed
    pub fn seed_lcg(&mut self, seed: u16) {
        self.lcg = seed as u32;
    }

    fn lcg_next(&mut self) -> i16 {
        self.lcg = 0x015A4E35u32.wrapping_mul(self.lcg).wrapping_add(1);
        ((self.lcg >> 16) & 0x7FFF) as i16
    }

    /// Tools_RandomLCG_Range
    pub fn lcg_range(&mut self, mut min: u16, mut max: u16) -> u16 {
        if min > max {
            std::mem::swap(&mut min, &mut max);
        }
        loop {
            let value = ((self.lcg_next() as i32 * (max as i32 - min as i32 + 1)) / 0x8000 + min as i32) as u16;
            if value <= max {
                return value;
            }
        }
    }
}
