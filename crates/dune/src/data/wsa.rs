//! WSA animations (wsa.c, codec/format40.c): delta-compressed frames XOR'd onto the previous one.

use super::format80;

pub struct Wsa {
    pub width: usize,
    pub height: usize,
    /// Decoded frames, each width*height indexed pixels.
    pub frames: Vec<Vec<u8>>,
    pub palette: Option<Vec<u8>>,
}

/// Format40_Decode: XOR-apply a delta onto `dst`.
fn format40_decode(dst: &mut [u8], src: &[u8]) {
    let mut d = 0usize;
    let mut s = 0usize;
    let get = |s: &mut usize| -> u8 {
        let v = src.get(*s).copied().unwrap_or(0);
        *s += 1;
        v
    };
    let mut xor = |d: &mut usize, v: u8| {
        if let Some(p) = dst.get_mut(*d) {
            *p ^= v;
        }
        *d += 1;
    };
    loop {
        if s >= src.len() {
            break;
        }
        let cmd = get(&mut s) as u16;
        if cmd == 0 {
            let count = get(&mut s);
            let v = get(&mut s);
            for _ in 0..count {
                xor(&mut d, v);
            }
        } else if cmd & 0x80 == 0 {
            for _ in 0..cmd {
                let v = get(&mut s);
                xor(&mut d, v);
            }
        } else if cmd != 0x80 {
            d += (cmd & 0x7F) as usize;
        } else {
            let cmd = get(&mut s) as u16 | (get(&mut s) as u16) << 8;
            if cmd == 0 {
                break;
            }
            if cmd & 0x8000 == 0 {
                d += cmd as usize;
            } else if cmd & 0x4000 == 0 {
                for _ in 0..(cmd & 0x3FFF) {
                    let v = get(&mut s);
                    xor(&mut d, v);
                }
            } else {
                let v = get(&mut s);
                for _ in 0..(cmd & 0x3FFF) {
                    xor(&mut d, v);
                }
            }
        }
    }
}

impl Wsa {
    pub fn load(data: &[u8]) -> Option<Wsa> {
        let le16 = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
        let le32 = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
        let frames = le16(0)? & 0x7FFF;
        let width = le16(2)?;
        let height = le16(4)?;
        let mut header_len = 10;
        let mut has_palette = le16(8)? != 0;
        let first = le32(10)?;
        let second = le32(14)?;
        if first != header_len + 8 + 4 * frames && second != header_len + 8 + 4 * frames {
            header_len = 8;
            has_palette = false;
        }
        let palette_len = if has_palette { 0x300 } else { 0 };
        let offset = |i: usize| le32(header_len + i * 4).unwrap_or(0);
        let palette = has_palette.then(|| {
            let start = header_len + (frames + 2) * 4;
            data.get(start..start + 0x300).map(|p| p.to_vec()).unwrap_or_default()
        });
        let mut screen = vec![0u8; width * height];
        let mut delta = vec![0u8; width * height * 2 + 1024];
        let mut out = Vec::with_capacity(frames);
        for i in 0..frames {
            let (start, end) = (offset(i), offset(i + 1));
            if start != 0 && end > start {
                let (a, b) = (start + palette_len, end + palette_len);
                if let Some(src) = data.get(a..b.min(data.len())) {
                    delta.fill(0);
                    let n = format80::decode(&mut delta, src);
                    format40_decode(&mut screen, &delta[..n]);
                }
            }
            out.push(screen.clone());
        }
        Some(Wsa { width, height, frames: out, palette })
    }
}
