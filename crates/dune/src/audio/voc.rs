//! Creative Voice File (.VOC) decoding: Dune II's digitised voices and sound effects.

/// Decoded 8-bit unsigned mono PCM.
#[derive(Clone, Debug, Default)]
pub struct Voc {
    pub rate: u32,
    pub samples: Vec<u8>,
}

/// Decode a VOC file. Handles sound data (1), continuation (2), silence (3), extended (8) and
/// new-format 8-bit PCM (9) blocks; anything else is skipped. Returns None for invalid files.
pub fn decode(data: &[u8]) -> Option<Voc> {
    if data.len() < 26 || !data.starts_with(b"Creative Voice File") {
        return None;
    }
    let mut pos = u16::from_le_bytes([data[20], data[21]]) as usize;
    let mut voc = Voc::default();
    let mut ext_rate: Option<u32> = None;
    while pos < data.len() {
        let kind = data[pos];
        if kind == 0 || pos + 4 > data.len() {
            break;
        }
        let len = (data[pos + 1] as usize) | (data[pos + 2] as usize) << 8 | (data[pos + 3] as usize) << 16;
        let body = &data[pos + 4..(pos + 4 + len).min(data.len())];
        match kind {
            1 if body.len() >= 2 => {
                if body[1] == 0 {
                    let rate = ext_rate.take().unwrap_or(1_000_000 / (256 - body[0] as u32));
                    if voc.rate == 0 {
                        voc.rate = rate;
                    }
                    voc.samples.extend_from_slice(&body[2..]);
                }
            }
            2 => voc.samples.extend_from_slice(body),
            3 if body.len() >= 3 => {
                let n = u16::from_le_bytes([body[0], body[1]]) as usize + 1;
                if voc.rate == 0 {
                    voc.rate = 1_000_000 / (256 - body[2] as u32);
                }
                voc.samples.extend(std::iter::repeat_n(0x80, n));
            }
            8 if body.len() >= 4 => {
                let tc = u16::from_le_bytes([body[0], body[1]]) as u32;
                let channels = body[3] as u32 + 1;
                ext_rate = Some(256_000_000 / ((65536 - tc) * channels));
            }
            9 if body.len() >= 12 => {
                let rate = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
                if body[4] == 8 && body[5] == 1 && voc.rate == 0 {
                    voc.rate = rate;
                }
                if body[4] == 8 && body[5] == 1 {
                    voc.samples.extend_from_slice(&body[12..]);
                }
            }
            _ => {}
        }
        pos += 4 + len;
    }
    (voc.rate != 0 && !voc.samples.is_empty()).then_some(voc)
}
