//! Westwood "format80" (LCW) decompressor, as used by CPS, SHP, ICN and WSA files.

/// Decode `src` into `dest`, stopping when `dest` is full or the end marker (0x80) is reached.
/// Returns the number of bytes written.
pub fn decode(dest: &mut [u8], src: &[u8]) -> usize {
    let end = dest.len();
    let mut d = 0usize;
    let mut s = 0usize;
    let next = |s: &mut usize| -> u8 {
        let v = src.get(*s).copied().unwrap_or(0x80);
        *s += 1;
        v
    };

    while d != end {
        let cmd = next(&mut s);
        if cmd == 0x80 {
            break;
        } else if cmd & 0x80 == 0 {
            // Short move, relative
            let size = ((cmd >> 4) as usize + 3).min(end - d);
            let offset = (((cmd & 0xF) as usize) << 8) | next(&mut s) as usize;
            for _ in 0..size {
                dest[d] = if offset <= d { dest[d - offset] } else { 0 };
                d += 1;
            }
        } else if cmd == 0xFE {
            // Long set
            let size = (next(&mut s) as usize | (next(&mut s) as usize) << 8).min(end - d);
            let v = next(&mut s);
            dest[d..d + size].fill(v);
            d += size;
        } else if cmd == 0xFF {
            // Long move, absolute
            let size = (next(&mut s) as usize | (next(&mut s) as usize) << 8).min(end - d);
            let mut offset = next(&mut s) as usize | (next(&mut s) as usize) << 8;
            for _ in 0..size {
                dest[d] = dest.get(offset).copied().unwrap_or(0);
                d += 1;
                offset += 1;
            }
        } else if cmd & 0x40 != 0 {
            // Short move, absolute
            let size = ((cmd & 0x3F) as usize + 3).min(end - d);
            let mut offset = next(&mut s) as usize | (next(&mut s) as usize) << 8;
            for _ in 0..size {
                dest[d] = dest.get(offset).copied().unwrap_or(0);
                d += 1;
                offset += 1;
            }
        } else {
            // Short copy
            let size = ((cmd & 0x3F) as usize).min(end - d);
            for _ in 0..size {
                dest[d] = next(&mut s);
                d += 1;
            }
        }
    }
    d
}
