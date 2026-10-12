//! Palettes, CPS full-screen images, ICN tile sets and SHP sprite sets.

use super::format80;

fn le16(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn le32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// 256-entry VGA palette, 6 bits per channel as stored in the files.
#[derive(Clone)]
pub struct Palette(pub [u8; 768]);

impl Palette {
    pub fn from_bytes(d: &[u8]) -> Palette {
        let mut p = [0u8; 768];
        let n = d.len().min(768);
        p[..n].copy_from_slice(&d[..n]);
        Palette(p)
    }

    /// Convert to 8-bit RGBA, the same way the VGA DAC scales 6-bit values.
    pub fn to_rgba(&self) -> [[u8; 4]; 256] {
        let mut out = [[0u8; 4]; 256];
        for i in 0..256 {
            let c = |v: u8| ((v & 0x3F) << 2) | ((v & 0x3F) >> 4);
            out[i] = [c(self.0[i * 3]), c(self.0[i * 3 + 1]), c(self.0[i * 3 + 2]), 255];
        }
        out
    }
}

/// Decode the "Sprites_Decode" container (type 0 = raw, type 4 = format80).
fn decode_container(src: &[u8], max: usize) -> Vec<u8> {
    match src.first() {
        Some(0) => {
            let size = le32(src, 2) as usize;
            let skip = le16(src, 6) as usize;
            let start = 8 + skip;
            src[start..(start + size).min(src.len())].to_vec()
        }
        Some(4) => {
            let skip = le16(src, 6) as usize;
            let mut out = vec![0u8; max];
            let n = format80::decode(&mut out, &src[8 + skip..]);
            out.truncate(n);
            out
        }
        _ => Vec::new(),
    }
}

/// A 320x200 CPS image plus its optional embedded palette.
pub struct Cps {
    pub pixels: Vec<u8>,
    pub palette: Option<Palette>,
}

pub fn load_cps(d: &[u8]) -> Cps {
    // u16 file size, u16 compression, u32 size, u16 palette size, palette, data
    let pal_size = le16(d, 8) as usize;
    let palette = (pal_size != 0).then(|| Palette::from_bytes(&d[10..10 + pal_size]));
    let mut hdr = Vec::with_capacity(d.len());
    hdr.extend_from_slice(&d[2..8]);
    hdr.extend_from_slice(&[0, 0]);
    hdr.extend_from_slice(&d[10 + pal_size..]);
    let mut pixels = decode_container(&hdr, 64000);
    pixels.resize(64000, 0);
    Cps { pixels, palette }
}

/// The landscape/structure tile set from ICON.ICN plus the ICON.MAP group table.
pub struct Tiles {
    pub width: usize,
    pub height: usize,
    /// 4 bits per pixel, `width*height/2` bytes per tile.
    pub pixels: Vec<u8>,
    /// tile id -> palette index into `rpal`.
    pub rtbl: Vec<u8>,
    /// 16-colour palettes.
    pub rpal: Vec<u8>,
    /// ICON.MAP contents.
    pub icon_map: Vec<u16>,
}

pub const ICM_ICONGROUP_WALLS: usize = 6;
pub const ICM_ICONGROUP_FOG_OF_WAR: usize = 7;
pub const ICM_ICONGROUP_CONCRETE_SLAB: usize = 8;
pub const ICM_ICONGROUP_LANDSCAPE: usize = 9;
pub const ICM_ICONGROUP_SPICE_BLOOM: usize = 10;

impl Tiles {
    pub fn load(icn: &[u8], map: &[u8]) -> Result<Tiles, String> {
        if &icn[0..4] != b"FORM" {
            return Err("ICON.ICN is not a FORM file".into());
        }
        let mut chunks = std::collections::HashMap::new();
        // FORM <len> ICON, then chunks
        let mut p = 12;
        while p + 8 <= icn.len() {
            let id = &icn[p..p + 4];
            let len = be32(icn, p + 4) as usize;
            let body = &icn[p + 8..(p + 8 + len).min(icn.len())];
            chunks.insert(String::from_utf8_lossy(id).to_string(), body);
            p += 8 + ((len + 1) & !1);
        }
        let sinf = chunks.get("SINF").ok_or("no SINF")?;
        let (w, h) = (sinf[0] as usize, sinf[1] as usize);
        let width = w << 3;
        let height = h << 3;
        let sset = chunks.get("SSET").ok_or("no SSET")?;
        let pixels = decode_container(sset, 1 << 20);
        let rtbl = chunks.get("RTBL").ok_or("no RTBL")?.to_vec();
        let rpal = chunks.get("RPAL").ok_or("no RPAL")?.to_vec();
        let icon_map = map.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Ok(Tiles { width, height, pixels, rtbl, rpal, icon_map })
    }

    pub fn count(&self) -> usize {
        self.pixels.len() / (self.width * self.height / 2)
    }

    /// Tile id of entry `index` inside icon group `group`.
    pub fn group(&self, group: usize, index: usize) -> u16 {
        self.icon_map[self.icon_map[group] as usize + index]
    }

    pub fn veiled_tile(&self) -> u16 {
        self.group(ICM_ICONGROUP_FOG_OF_WAR, 16)
    }
    pub fn bloom_tile(&self) -> u16 {
        self.group(ICM_ICONGROUP_SPICE_BLOOM, 0)
    }
    pub fn built_slab_tile(&self) -> u16 {
        self.group(ICM_ICONGROUP_CONCRETE_SLAB, 2)
    }
    pub fn landscape_tile(&self) -> u16 {
        self.group(ICM_ICONGROUP_LANDSCAPE, 0)
    }
    pub fn wall_tile(&self) -> u16 {
        self.group(ICM_ICONGROUP_WALLS, 0)
    }
}

/// One decoded SHP sprite. `pixels` keeps the original encoding where 0 is followed by a
/// transparent run length; `remap` is the optional 16-entry sprite palette.
#[derive(Clone)]
pub struct Sprite {
    pub width: u16,
    pub height: u8,
    pub flags: u16,
    pub remap: Option<[u8; 16]>,
    pub data: Vec<u8>,
}

impl Sprite {
    /// Expand the run-length-of-zero encoding into a width*height index buffer (0 = transparent).
    pub fn expand(&self) -> Vec<u8> {
        let w = self.width as usize;
        let h = self.height as usize;
        let mut out = vec![0u8; w * h];
        let mut s = 0usize;
        for y in 0..h {
            let mut x = 0usize;
            while x < w && s < self.data.len() {
                let v = self.data[s];
                s += 1;
                if v == 0 {
                    x += *self.data.get(s).unwrap_or(&0) as usize;
                    s += 1;
                } else {
                    let v = match &self.remap {
                        Some(r) => r[v as usize & 0xF],
                        None => v,
                    };
                    out[y * w + x] = v;
                    x += 1;
                }
            }
        }
        out
    }
}

pub fn load_shp(d: &[u8]) -> Vec<Option<Sprite>> {
    let count = le16(d, 0) as usize;
    let old_format = (4 + count as u32 * 4) != le32(d, 2);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let offset = if old_format { le16(d, 2 + 2 * i) as usize } else { le32(d, 2 + 4 * i) as usize };
        if offset == 0 || offset >= d.len() {
            out.push(None);
            continue;
        }
        let src = &d[offset + if old_format { 0 } else { 2 }..];
        let flags = le16(src, 0);
        let height = src[2];
        let width = le16(src, 3);
        let decoded_size = le16(src, 8) as usize;
        let mut p = 10;
        let remap = if flags & 1 != 0 {
            let mut r = [0u8; 16];
            r.copy_from_slice(&src[p..p + 16]);
            p += 16;
            Some(r)
        } else {
            None
        };
        let data = if flags & 2 == 0 {
            let mut buf = vec![0u8; decoded_size];
            format80::decode(&mut buf, &src[p..]);
            buf
        } else {
            let packed = le16(src, 6) as usize;
            src[p..packed.min(src.len())].to_vec()
        };
        out.push(Some(Sprite { width, height, flags, remap, data }));
    }
    out
}
