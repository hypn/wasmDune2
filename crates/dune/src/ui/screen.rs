//! The 320x200 indexed screen and the original drawing primitives (gfx.c, gui.c, font.c).

use crate::data::image::{Sprite, Tiles};
use crate::data::pak::Vfs;

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;

pub const DRAWSPRITE_FLAG_RTL: u16 = 0x0001;
pub const DRAWSPRITE_FLAG_BOTTOMUP: u16 = 0x0002;
pub const DRAWSPRITE_FLAG_ZOOM: u16 = 0x0004;
pub const DRAWSPRITE_FLAG_REMAP: u16 = 0x0100;
pub const DRAWSPRITE_FLAG_BLUR: u16 = 0x0200;
pub const DRAWSPRITE_FLAG_SPRITEPAL: u16 = 0x0400;
pub const DRAWSPRITE_FLAG_PAL: u16 = 0x2000;
pub const DRAWSPRITE_FLAG_WIDGETPOS: u16 = 0x4000;
pub const DRAWSPRITE_FLAG_CENTER: u16 = 0x8000;

/// g_widgetProperties: (xBase/8, yBase, width/8, height, fgColourBlink, fgColourNormal, fgColourSelected)
pub const WIDGET_PROPERTIES: [(u16, u16, u16, u16, u8, u8, u8); 22] = [
    (0, 0, 40, 200, 15, 12, 0),
    (1, 75, 29, 70, 15, 15, 0),
    (0, 40, 30, 160, 15, 20, 0),
    (32, 136, 8, 64, 15, 12, 0),
    (32, 44, 8, 9, 29, 116, 0),
    (32, 4, 8, 9, 29, 116, 0),
    (32, 42, 8, 82, 15, 20, 0),
    (1, 21, 38, 14, 12, 116, 0),
    (16, 48, 23, 112, 15, 233, 0),
    (2, 176, 36, 11, 15, 20, 0),
    (0, 40, 40, 160, 29, 20, 0),
    (16, 48, 23, 112, 29, 20, 0),
    (9, 80, 22, 112, 29, 116, 0),
    (12, 140, 16, 42, 236, 233, 0),
    (2, 89, 36, 60, 0, 0, 0),
    (4, 110, 32, 12, 232, 235, 0),
    (5, 48, 30, 134, 0, 0, 0),
    (3, 36, 36, 148, 0, 0, 0),
    (1, 72, 38, 52, 0, 0, 0),
    (0, 0, 0, 0, 0, 0, 0),
    (2, 24, 36, 152, 12, 12, 0),
    (1, 6, 12, 3, 0, 15, 6),
];

const COLOUR_BORDER_SCHEMA: [[u8; 4]; 5] = [[26, 29, 29, 29], [20, 26, 16, 20], [20, 16, 26, 20], [233, 235, 232, 233], [233, 232, 235, 233]];

pub struct FontChar {
    pub width: u8,
    pub unused_lines: u8,
    pub used_lines: u8,
    pub data: Option<Vec<u8>>,
}

pub struct Font {
    pub height: u8,
    pub chars: Vec<FontChar>,
}

impl Font {
    /// Font_LoadFile
    pub fn load(buf: &[u8]) -> Option<Font> {
        if buf.len() < 14 || buf[2] != 0x00 || buf[3] != 0x05 {
            return None;
        }
        let le = |o: usize| u16::from_le_bytes([buf[o], buf[o + 1]]) as usize;
        let start = le(4);
        let data_start = le(6);
        let width_list = le(8);
        let line_list = le(12);
        let height = buf[start + 4];
        let count = le(10) - width_list;
        let mut chars = Vec::with_capacity(count);
        for i in 0..count {
            let width = buf[width_list + i];
            let unused_lines = buf[line_list + i * 2];
            let used_lines = buf[line_list + i * 2 + 1];
            let off = le(data_start + i * 2);
            let data = (off != 0).then(|| {
                let mut d = vec![0u8; used_lines as usize * width as usize];
                for y in 0..used_lines as usize {
                    for x in 0..width as usize {
                        let mut v = buf[off + y * ((width as usize + 1) / 2) + x / 2];
                        if x % 2 != 0 {
                            v >>= 4;
                        }
                        d[y * width as usize + x] = v & 0xF;
                    }
                }
                d
            });
            chars.push(FontChar { width, unused_lines, used_lines, data });
        }
        Some(Font { height, chars })
    }
}

pub struct Fonts {
    pub new6p: Option<Font>,
    pub new8p: Option<Font>,
}

impl Fonts {
    pub fn load(vfs: &Vfs) -> Fonts {
        Fonts { new6p: vfs.get("NEW6P.FNT").and_then(Font::load), new8p: vfs.get("NEW8P.FNT").and_then(Font::load) }
    }
}

#[derive(Clone, Copy)]
pub struct Clip {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

/// The drawing context: one 320x200 screen plus the original's drawing state.
pub struct Screen {
    pub pixels: Vec<u8>,
    pub clip: Clip,
    /// g_colours: text colour table.
    pub colours: [u8; 16],
    /// Which font is selected (true = 6p).
    pub font_6p: bool,
    pub font_char_offset: i8,
    blur_index: u16,
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    pub fn new() -> Screen {
        Screen {
            pixels: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT],
            clip: Clip { left: 0, top: 0, right: SCREEN_WIDTH as i16 - 1, bottom: SCREEN_HEIGHT as i16 - 1 },
            colours: [0; 16],
            font_6p: false,
            font_char_offset: -1,
            blur_index: 0,
        }
    }

    #[inline]
    pub fn put_pixel(&mut self, x: i32, y: i32, c: u8) {
        if x < 0 || y < 0 || x >= SCREEN_WIDTH as i32 || y >= SCREEN_HEIGHT as i32 {
            return;
        }
        self.pixels[y as usize * SCREEN_WIDTH + x as usize] = c;
    }

    /// GUI_DrawFilledRectangle
    pub fn fill_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, colour: u8) {
        if left >= SCREEN_WIDTH as i32 || top >= SCREEN_HEIGHT as i32 {
            return;
        }
        let (l, t) = (left.max(0), top.max(0));
        let (r, b) = (right.clamp(0, SCREEN_WIDTH as i32 - 1), bottom.clamp(0, SCREEN_HEIGHT as i32 - 1));
        if l > r || t > b {
            return;
        }
        for y in t..=b {
            let row = y as usize * SCREEN_WIDTH;
            self.pixels[row + l as usize..=row + r as usize].fill(colour);
        }
    }

    /// GUI_DrawXorFilledRectangle
    pub fn xor_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, colour: u8) {
        let (l, t) = (left.max(0), top.max(0));
        let (r, b) = (right.clamp(0, SCREEN_WIDTH as i32 - 1), bottom.clamp(0, SCREEN_HEIGHT as i32 - 1));
        if l > r || t > b {
            return;
        }
        for y in t..=b {
            for x in l..=r {
                self.pixels[y as usize * SCREEN_WIDTH + x as usize] ^= colour;
            }
        }
    }

    /// GUI_DrawLine (Bresenham as in the original; clipping is done per pixel).
    pub fn line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, colour: u8) {
        let c = self.clip;
        let plot = |s: &mut Screen, x: i32, y: i32| {
            if x >= c.left as i32 && x <= c.right as i32 && y >= c.top as i32 && y <= c.bottom as i32 {
                s.put_pixel(x, y, colour);
            }
        };
        let (mut x1, mut y1, mut x2, mut y2) = (x1, y1, x2, y2);
        y2 -= y1;
        if y2 == 0 {
            if x1 >= x2 {
                std::mem::swap(&mut x1, &mut x2);
            }
            for x in x1..=x2 {
                plot(self, x, y1);
            }
            return;
        }
        if y2 < 0 {
            std::mem::swap(&mut x1, &mut x2);
            y2 = -y2;
            y1 -= y2;
        }
        x2 -= x1;
        if x2 == 0 {
            for y in 0..y2 {
                plot(self, x1, y1 + y);
            }
            return;
        }
        let mut increment = 1;
        if x2 < 0 {
            x2 = -x2;
            increment = -1;
        }
        let (mut x, mut y) = (x1, y1);
        if x2 < y2 {
            let full = y2;
            let mut half = y2 / 2;
            let mut n = y2;
            loop {
                plot(self, x, y);
                if n == 0 {
                    return;
                }
                n -= 1;
                y += 1;
                half -= x2;
                if half < 0 {
                    half += full;
                    x += increment;
                }
            }
        } else {
            let full = x2;
            let mut half = x2 / 2;
            let mut n = x2;
            loop {
                plot(self, x, y);
                if n == 0 {
                    return;
                }
                n -= 1;
                x += increment;
                half -= y2;
                if half < 0 {
                    half += full;
                    y += 1;
                }
            }
        }
    }

    /// GUI_DrawWiredRectangle
    pub fn wired_rect(&mut self, left: i32, top: i32, right: i32, bottom: i32, colour: u8) {
        self.line(left, top, right, top, colour);
        self.line(left, bottom, right, bottom, colour);
        self.line(left, top, left, bottom, colour);
        self.line(right, top, right, bottom, colour);
    }

    /// GUI_DrawBorder
    pub fn border(&mut self, left: i32, top: i32, width: i32, height: i32, schema: usize, fill: bool) {
        let w = width - 1;
        let h = height - 1;
        let cs = COLOUR_BORDER_SCHEMA[schema];
        if fill {
            self.fill_rect(left, top, left + w, top + h, cs[0]);
        }
        self.line(left, top + h, left + w, top + h, cs[1]);
        self.line(left + w, top, left + w, top + h, cs[1]);
        self.line(left, top, left + w, top, cs[2]);
        self.line(left, top, left, top + h, cs[2]);
        self.put_pixel(left, top + h, cs[3]);
        self.put_pixel(left + w, top, cs[3]);
    }

    /// GUI_Widget_DrawBorder
    pub fn widget_border(&mut self, widget_index: usize, border_type: usize, pressed: bool) {
        const BORDER_INDEX_SIZE: [[i32; 2]; 4] = [[0, 0], [2, 4], [1, 1], [2, 1]];
        let wp = WIDGET_PROPERTIES[widget_index];
        let (left, top, width, height) = ((wp.0 * 8) as i32, wp.1 as i32, (wp.2 * 8) as i32, wp.3 as i32);
        let mut schema = if pressed { 2 } else { 0 };
        self.border(left, top, width, height, schema + 1, true);
        let size = BORDER_INDEX_SIZE[border_type][1];
        if size != 0 {
            schema += BORDER_INDEX_SIZE[border_type][0] as usize;
            self.border(left + size, top + size, width - size * 2, height - size * 2, schema, false);
        }
    }

    /// GUI_DrawBlockedRectangle
    pub fn blocked_rect(&mut self, left: i32, top: i32, width: i32, height: i32, colour: u8) {
        for y in 0..height {
            for x in 0..width {
                if (x + y) % 2 == 0 {
                    self.put_pixel(left + x, top + y, colour);
                }
            }
        }
    }

    /// GFX_DrawTile
    pub fn draw_tile(&mut self, tiles: &Tiles, tile_id: u16, x: i32, y: i32, house_id: u8, enhanced: bool) {
        let tid = tile_id as usize;
        if tid >= tiles.count() {
            return;
        }
        let pal_index = tiles.rtbl[tid] as usize;
        let mut pal = [0u8; 16];
        pal.copy_from_slice(&tiles.rpal[pal_index * 16..pal_index * 16 + 16]);
        if house_id != 0 {
            for c in pal.iter_mut() {
                if *c & 0xF0 == 0x90 && (*c <= 0x96 || !enhanced) {
                    *c = c.wrapping_add(house_id << 4);
                }
            }
        }
        let transparent = pal[0] == 0;
        let (w, h) = (tiles.width, tiles.height);
        let base = tid * w * h / 2;
        for j in 0..h {
            let sy = y + j as i32;
            if sy < 0 || sy >= SCREEN_HEIGHT as i32 {
                continue;
            }
            for i in 0..w {
                let sx = x + i as i32;
                if sx < 0 || sx >= SCREEN_WIDTH as i32 {
                    continue;
                }
                let b = tiles.pixels[base + (j * w + i) / 2];
                let c = pal[if i & 1 == 0 { (b >> 4) as usize } else { (b & 0xF) as usize }];
                if transparent && c == 0 {
                    continue;
                }
                self.pixels[sy as usize * SCREEN_WIDTH + sx as usize] = c;
            }
        }
    }

    /// GUI_DrawSprite. `window` is a g_widgetProperties index. `remap` = (table, count).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_sprite(&mut self, sprite: Option<&Sprite>, pos_x: i32, pos_y: i32, window: usize, flags: u16, palette: Option<&[u8; 16]>, remap: Option<(&[u8; 256], u16)>) {
        let Some(sprite) = sprite else { return };
        const BLUR_OFFSETS: [i32; 8] = [1, 3, 2, 5, 4, 3, 2, 1];
        let mut flags = flags;
        let mut blur_offset = 1i32;
        let mut blur_incr: u16 = 0x8B55;
        let mut blur_value: u16 = 0x51EC;
        if flags & DRAWSPRITE_FLAG_BLUR != 0 {
            self.blur_index = (self.blur_index + 1) % 8;
            blur_offset = BLUR_OFFSETS[self.blur_index as usize];
            blur_value = 0;
            blur_incr = 0x100;
        }
        let (remap_table, remap_count) = match remap {
            Some((t, c)) if flags & DRAWSPRITE_FLAG_REMAP != 0 && c != 0 => (Some(t), c),
            _ => {
                flags &= !DRAWSPRITE_FLAG_REMAP;
                (None, 0)
            }
        };
        let wp = WIDGET_PROPERTIES[window];
        let win_x = (wp.0 * 8) as i32;
        let width = (wp.2 * 8) as i32;
        let top = wp.1 as i32;
        let bottom = top + wp.3 as i32;
        let mut pos_x = pos_x;
        let mut pos_y = pos_y;
        if flags & DRAWSPRITE_FLAG_WIDGETPOS != 0 {
            pos_y += top;
        } else {
            pos_x -= win_x;
        }
        let palette: Option<&[u8; 16]> = if sprite.flags & 1 != 0 {
            flags |= DRAWSPRITE_FLAG_SPRITEPAL;
            if flags & DRAWSPRITE_FLAG_PAL != 0 {
                palette
            } else {
                sprite.remap.as_ref()
            }
        } else {
            palette
        };
        let sw = sprite.width as i32;
        let sh = sprite.height as i32;
        if flags & DRAWSPRITE_FLAG_CENTER != 0 {
            pos_x -= sw / 2;
            pos_y -= sh / 2;
        }
        let rtl = flags & DRAWSPRITE_FLAG_RTL != 0;
        let bottomup = flags & DRAWSPRITE_FLAG_BOTTOMUP != 0;
        let mode = flags & 0x0700;

        // Decode the RLE rows once.
        let mut rows: Vec<Vec<u8>> = Vec::with_capacity(sh as usize);
        {
            let d = &sprite.data;
            let mut s = 0usize;
            for _ in 0..sh {
                let mut row = vec![0u8; sw as usize];
                let mut x = 0i32;
                while x < sw && s < d.len() {
                    let v = d[s];
                    s += 1;
                    if v == 0 {
                        x += *d.get(s).unwrap_or(&0) as i32;
                        s += 1;
                    } else {
                        if x >= 0 && x < sw {
                            row[x as usize] = v;
                        }
                        x += 1;
                    }
                }
                rows.push(row);
            }
        }

        for row_i in 0..sh {
            let src_row = &rows[row_i as usize];
            let sy = if bottomup { pos_y + sh - 1 - row_i } else { pos_y + row_i };
            if sy < top || sy >= bottom || sy < 0 || sy >= SCREEN_HEIGHT as i32 {
                continue;
            }
            for col in 0..sw {
                let v = src_row[col as usize];
                if v == 0 {
                    continue;
                }
                let dx = if rtl { pos_x + sw - 1 - col } else { pos_x + col };
                if dx < 0 || dx >= width {
                    continue;
                }
                let x = win_x + dx;
                if x < 0 || x >= SCREEN_WIDTH as i32 {
                    continue;
                }
                let idx = sy as usize * SCREEN_WIDTH + x as usize;
                let pal = |v: u8| palette.map(|p| p[(v & 0xF) as usize]).unwrap_or(v);
                let do_remap = |mut v: u8| {
                    if let Some(t) = remap_table {
                        for _ in 0..remap_count {
                            v = t[v as usize];
                        }
                    }
                    v
                };
                match mode {
                    0 => self.pixels[idx] = v,
                    DRAWSPRITE_FLAG_REMAP => self.pixels[idx] = do_remap(v),
                    DRAWSPRITE_FLAG_BLUR => {
                        blur_value = blur_value.wrapping_add(blur_incr);
                        if blur_value & 0xFF00 == 0 {
                            self.pixels[idx] = v;
                        } else {
                            blur_value &= 0xFF;
                            let o = idx as i32 + blur_offset;
                            if (o as usize) < self.pixels.len() {
                                self.pixels[idx] = self.pixels[o as usize];
                            }
                        }
                    }
                    m if m == DRAWSPRITE_FLAG_REMAP | DRAWSPRITE_FLAG_BLUR || m == DRAWSPRITE_FLAG_REMAP | DRAWSPRITE_FLAG_BLUR | DRAWSPRITE_FLAG_SPRITEPAL => {
                        // Shadow: remap what is already on screen.
                        self.pixels[idx] = do_remap(self.pixels[idx]);
                    }
                    DRAWSPRITE_FLAG_SPRITEPAL => self.pixels[idx] = pal(v),
                    m if m == DRAWSPRITE_FLAG_REMAP | DRAWSPRITE_FLAG_SPRITEPAL => self.pixels[idx] = do_remap(pal(v)),
                    m if m == DRAWSPRITE_FLAG_BLUR | DRAWSPRITE_FLAG_SPRITEPAL => {
                        blur_value = blur_value.wrapping_add(blur_incr);
                        if blur_value & 0xFF00 == 0 {
                            self.pixels[idx] = pal(v);
                        } else {
                            blur_value &= 0xFF;
                            let o = idx as i32 + blur_offset;
                            if (o as usize) < self.pixels.len() {
                                self.pixels[idx] = self.pixels[o as usize];
                            }
                        }
                    }
                    _ => self.pixels[idx] = v,
                }
            }
        }
    }

    /// GUI_InitColors
    pub fn init_colours(&mut self, colours: &[u8], first: usize, last: usize) {
        for i in first..=last.min(15) {
            if let Some(&c) = colours.get(i - first) {
                self.colours[i] = c;
            }
        }
    }

    fn font<'a>(&self, fonts: &'a Fonts) -> Option<&'a Font> {
        if self.font_6p {
            fonts.new6p.as_ref().or(fonts.new8p.as_ref())
        } else {
            fonts.new8p.as_ref()
        }
    }

    /// Font_GetCharWidth
    pub fn char_width(&self, fonts: &Fonts, c: u8) -> i32 {
        self.font(fonts).and_then(|f| f.chars.get(c as usize)).map(|ch| ch.width as i32 + self.font_char_offset as i32).unwrap_or(0)
    }

    /// Font_GetStringWidth
    pub fn string_width(&self, fonts: &Fonts, s: &[u8]) -> i32 {
        s.iter().map(|&c| self.char_width(fonts, c)).sum()
    }

    pub fn font_height(&self, fonts: &Fonts) -> i32 {
        self.font(fonts).map(|f| f.height as i32).unwrap_or(8)
    }

    /// GUI_DrawChar
    fn draw_char(&mut self, fonts: &Fonts, c: u8, x: i32, y: i32) {
        let Some(font) = self.font(fonts) else { return };
        let Some(fc) = font.chars.get(c as usize) else { return };
        let Some(data) = &fc.data else { return };
        let (w, h) = (fc.width as i32, font.height as i32);
        if x < 0 || x + w > SCREEN_WIDTH as i32 || y < 0 || y + h > SCREEN_HEIGHT as i32 {
            return;
        }
        let bg = self.colours[0];
        let mut yy = y;
        for _ in 0..fc.unused_lines {
            if bg != 0 {
                for i in 0..w {
                    self.pixels[yy as usize * SCREEN_WIDTH + (x + i) as usize] = bg;
                }
            }
            yy += 1;
        }
        for j in 0..fc.used_lines as i32 {
            for i in 0..w {
                let c = self.colours[(data[(j * w + i) as usize] & 0xF) as usize];
                if c != 0 {
                    self.pixels[yy as usize * SCREEN_WIDTH + (x + i) as usize] = c;
                }
            }
            yy += 1;
        }
        if bg != 0 {
            for _ in (fc.unused_lines + fc.used_lines) as i32..h {
                for i in 0..w {
                    self.pixels[yy as usize * SCREEN_WIDTH + (x + i) as usize] = bg;
                }
                yy += 1;
            }
        }
    }

    /// GUI_DrawText
    pub fn draw_text(&mut self, fonts: &Fonts, s: &[u8], left: i32, top: i32, fg: u8, bg: u8) {
        let left = left.max(0);
        let top = top.max(0);
        if left > SCREEN_WIDTH as i32 || top > SCREEN_HEIGHT as i32 {
            return;
        }
        self.colours[0] = bg;
        self.colours[1] = fg;
        let h = self.font_height(fonts);
        let (mut x, mut y) = (left, top);
        let mut i = 0;
        while i < s.len() && s[i] != 0 {
            if s[i] == b'\n' || s[i] == b'\r' {
                x = left;
                y += h;
                while i < s.len() && (s[i] == b'\n' || s[i] == b'\r') {
                    i += 1;
                }
                if i >= s.len() {
                    break;
                }
            }
            let w = self.char_width(fonts, s[i]);
            if x + w > SCREEN_WIDTH as i32 {
                x = left;
                y += h;
            }
            if y > SCREEN_HEIGHT as i32 {
                break;
            }
            self.draw_char(fonts, s[i], x, y);
            x += w;
            i += 1;
        }
    }

    /// GUI_DrawText_Wrapper (without the printf: format first with game::format_c).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text_wrapper(&mut self, fonts: &Fonts, s: &[u8], left: i32, top: i32, fg: u8, bg: u8, flags: u16) {
        let font = flags & 0x0F;
        if font != 0 {
            self.font_6p = font == 1;
        }
        let style = flags & 0xF0;
        if style != 0 {
            let mut colours = [0u8; 16];
            match style {
                0x10 => {
                    colours[2] = 0;
                    colours[3] = 0;
                    self.font_char_offset = -2;
                }
                0x20 => {
                    colours[2] = 12;
                    colours[3] = 0;
                    self.font_char_offset = -1;
                }
                0x30 => {
                    colours[2] = 12;
                    colours[3] = 12;
                    self.font_char_offset = -1;
                }
                0x40 => {
                    colours[2] = 232;
                    colours[3] = 0;
                    self.font_char_offset = -1;
                }
                _ => {}
            }
            colours[0] = bg;
            colours[1] = fg;
            colours[4] = 6;
            self.colours = colours;
        }
        let mut left = left;
        match flags & 0x0F00 {
            0x100 => left -= self.string_width(fonts, s) / 2,
            0x200 => left -= self.string_width(fonts, s),
            _ => {}
        }
        self.draw_text(fonts, s, left, top, fg, bg);
    }

    /// GUI_Palette_RemapScreen
    pub fn remap_area(&mut self, left: usize, top: usize, width: usize, height: usize, remap: &[u8; 256]) {
        for y in top..(top + height).min(SCREEN_HEIGHT) {
            for x in left..(left + width).min(SCREEN_WIDTH) {
                let p = &mut self.pixels[y * SCREEN_WIDTH + x];
                *p = remap[*p as usize];
            }
        }
    }
}

/// GUI_Palette_CreateMapping: for each colour, the closest colour shifted towards `reference`.
pub fn palette_create_mapping(palette: &[u8; 768], reference: u8, intensity: u8) -> [u8; 256] {
    let mut colours = [0u8; 256];
    let p = |i: usize| palette[i] as i32;
    let r = reference as usize;
    for index in 1..256usize {
        let f = (intensity / 2) as i32;
        let red = (p(3 * index) - (((p(3 * index) - p(3 * r)) * f) >> 7)) as u8 as i32;
        let blue = (p(3 * index + 1) - (((p(3 * index + 1) - p(3 * r + 1)) * f) >> 7)) as u8 as i32;
        let green = (p(3 * index + 2) - (((p(3 * index + 2) - p(3 * r + 2)) * f) >> 7)) as u8 as i32;
        let mut colour = reference;
        let mut sum_min: u32 = 0xFFFF;
        for i in 1..256usize {
            let sum = ((p(3 * i) - red).pow(2) + (p(3 * i + 1) - blue).pow(2) + (p(3 * i + 2) - green).pow(2)) as u32 & 0xFFFF;
            if sum > sum_min {
                continue;
            }
            if i != r && i == index {
                continue;
            }
            sum_min = sum;
            colour = i as u8;
        }
        colours[index] = colour;
    }
    colours
}

/// GUI_Palette_CreateRemap: interface colours 0x90..0x96 to the house colour.
pub fn palette_create_remap(house_id: u8) -> [u8; 256] {
    let mut remap = [0u8; 256];
    for (i, r) in remap.iter_mut().enumerate() {
        *r = i as u8;
        if i / 16 == 9 && i % 16 <= 6 {
            *r = (house_id << 4) + 0x90 + (i % 16) as u8;
        }
    }
    remap
}
