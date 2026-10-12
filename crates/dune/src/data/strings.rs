//! In-game text: String_Init / String_Load from the .ENG files, with the same index layout.

use super::pak::Vfs;
use crate::tables::strings::*;

const COUPLES: &[u8] = b" etainosrlhcdupmtasio wb rnsdalmh ieorasnrtlc synstcloer dtgesionr ufmsw tep.icae oiadur laeiyodeia otruetoakhlr eiu,.oansrctlaileoiratpeaoip bm";

fn decompress(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() && src[i] != 0 {
        let mut c = src[i];
        if c & 0x80 != 0 {
            c &= 0x7F;
            out.push(COUPLES[(c >> 3) as usize]);
            c = COUPLES[c as usize + 16];
        } else if c == 0x1B {
            i += 1;
            c = 0x7Fu8.wrapping_add(*src.get(i).unwrap_or(&0));
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Convert the game's code page (CP437-like) to a Rust string for display/debugging.
/// Glyph rendering uses the raw bytes, so this is only lossy for non-ASCII.
fn to_string(b: &[u8]) -> String {
    b.iter().map(|&c| if c < 0x80 { c as char } else { '?' }).collect()
}

pub struct Strings {
    /// Raw bytes per string id (font glyph indices).
    pub raw: Vec<Vec<u8>>,
}

impl Strings {
    pub fn load(vfs: &Vfs) -> Strings {
        let mut s = Strings { raw: Vec::new() };
        s.load_file(vfs, "DUNE", false, 339);
        s.load_file(vfs, "MESSAGE", false, 367);
        s.load_file(vfs, "INTRO", false, 404);
        s.load_file(vfs, "TEXTH", true, 444);
        s.load_file(vfs, "TEXTA", true, 484);
        s.load_file(vfs, "TEXTO", true, 524);
        s.load_file(vfs, "PROTECT", true, 0);
        s
    }

    fn load_file(&mut self, vfs: &Vfs, name: &str, compressed: bool, end: usize) {
        let Some(buf) = vfs.get(&format!("{name}.ENG")) else {
            while end != 0 && self.raw.len() <= end {
                self.raw.push(Vec::new());
            }
            return;
        };
        let count = u16::from_le_bytes([buf[0], buf[1]]) as usize / 2;
        let end = if end == 0 { self.raw.len() + count - 1 } else { end };
        for i in 0..count {
            if self.raw.len() > end {
                break;
            }
            let off = u16::from_le_bytes([buf[i * 2], buf[i * 2 + 1]]) as usize;
            let src = &buf[off.min(buf.len())..];
            let mut text = if compressed {
                decompress(src)
            } else {
                src.iter().take_while(|&&c| c != 0).copied().collect()
            };
            while text.last().is_some_and(|c| c.is_ascii_whitespace()) {
                text.pop();
            }
            // Empty strings are skipped, except the very first slot (as the original does).
            if !text.is_empty() || self.raw.is_empty() {
                self.raw.push(text);
            }
        }
        if name == "DUNE" && self.raw.len() == 332 && self.raw.get(44).is_some_and(|s| s == b"Pre-spice mound") {
            self.remap_demo();
        }
        if self.raw.len() == 335 {
            // Dune II v1.0 message file: the original duplicates entries 281/282 to shift the rest.
            let (a, b) = (self.raw[281].clone(), self.raw[282].clone());
            self.raw.insert(283, b);
            self.raw.insert(283, a);
            let null = self.raw[STR_NULL as usize].clone();
            let old = self.raw[STR_OLD_SAVE_GAME_FILE_IS_INCOMPATABLE_WITH_LATEST_VERSION as usize].clone();
            self.raw.push(null);
            self.raw.push(old);
        }
        if self.raw.len() == STR_LOAD_GAME as usize {
            let s = self.raw[STR_LOAD_A_GAME as usize].clone();
            self.raw.push(s);
        }
        while self.raw.len() <= end {
            self.raw.push(Vec::new());
        }
    }

    /// The Nov 1992 shareware DUNE.ENG lacks "Saboteur" (id 44), has a single intro line where v1.0
    /// has two, carries one extra line, and stops before the Hall of Fame strings. Rebuild the
    /// v1.0 layout (as after the duplication below) so every STR_* id points at the right text.
    fn remap_demo(&mut self) {
        let d = std::mem::take(&mut self.raw);
        let lit = |s: &str| s.as_bytes().to_vec();
        let mut r: Vec<Vec<u8>> = d[..44].to_vec();
        r.push(lit("Saboteur"));
        r.extend_from_slice(&d[44..282]); // .. 280, 281: the intro lines
        r.push(d[280].clone());
        r.push(d[281].clone());
        r.extend_from_slice(&d[282..326]); // .. 328
        r.extend_from_slice(&d[327..332]); // 329 .. 333 (326 is a demo-only line)
        for s in ["Hall of Fame", "SelectPlace toHarvest.", "Hall of Fame", "", "Old save game file is incompatable with latest version.", "Load a game"] {
            r.push(lit(s));
        }
        self.raw = r;
    }

    pub fn get(&self, id: u16) -> &[u8] {
        self.raw.get(id as usize).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn text(&self, id: u16) -> String {
        to_string(self.get(id))
    }
}
