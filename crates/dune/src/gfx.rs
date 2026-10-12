//! Software rendering into an indexed 8-bit screen, like the original's VGA buffers.

use crate::data::image::Tiles;

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;

/// An indexed-colour surface.
pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Surface {
    pub fn new(width: usize, height: usize) -> Self {
        Surface { width, height, pixels: vec![0; width * height] }
    }

    /// GFX_DrawTile, with clipping so it can also be used on larger off-screen surfaces.
    pub fn draw_tile(&mut self, tiles: &Tiles, tile_id: u16, x: i32, y: i32, house_id: u8) {
        let tid = tile_id as usize;
        if tid >= tiles.count() {
            return;
        }
        let pal_index = tiles.rtbl[tid] as usize;
        let mut pal = [0u8; 16];
        pal.copy_from_slice(&tiles.rpal[pal_index * 16..pal_index * 16 + 16]);
        if house_id != 0 {
            for c in pal.iter_mut() {
                // ENHANCEMENT (OpenDUNE): only recolour 0x90..=0x96 to avoid IX building glitches.
                if *c & 0xF0 == 0x90 && *c <= 0x96 {
                    *c += house_id << 4;
                }
            }
        }
        let transparent = pal[0] == 0;
        let (w, h) = (tiles.width, tiles.height);
        let base = tid * w * h / 2;
        for j in 0..h {
            let sy = y + j as i32;
            if sy < 0 || sy >= self.height as i32 {
                continue;
            }
            for i in 0..w {
                let sx = x + i as i32;
                if sx < 0 || sx >= self.width as i32 {
                    continue;
                }
                let b = tiles.pixels[base + (j * w + i) / 2];
                let c = pal[if i & 1 == 0 { (b >> 4) as usize } else { (b & 0xF) as usize }];
                if transparent && c == 0 {
                    continue;
                }
                self.pixels[sy as usize * self.width + sx as usize] = c;
            }
        }
    }

    pub fn to_rgba(&self, palette: &[[u8; 4]; 256], out: &mut [u8]) {
        for (i, &p) in self.pixels.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&palette[p as usize]);
        }
    }
}
