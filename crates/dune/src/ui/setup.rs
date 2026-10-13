//! The skirmish setup screen (new: the original had no skirmish mode).
//!
//! Laid out and handled like the original's option windows (GUI_Window_Create, e.g. "Game
//! Controls"): a window over the shaded interface with labelled buttons that cycle their value
//! when clicked (right click cycles backwards). It is dressed in the interface's own art from
//! SCREEN.CPS: its marbled sand, MENTAT/OPTIONS style buttons and blue lettering.
//! Beside the settings is a radar-style preview of the map.

use super::screen::*;
use super::GameUi;
use crate::game::skirmish::{AiDifficulty, SkirmishSettings};
use crate::game::*;
use crate::tables::landscape::LANDSCAPE_INFO;
use crate::tables::HOUSE_INFO;
use crate::tile::{packed_x, packed_y};

// The window, in screen pixels.
const WIN_X: i32 = 16;
const WIN_Y: i32 = 10;
const WIN_W: i32 = 288;
const WIN_H: i32 = 180;
const ROW_Y: i32 = 24;
const ROW_STEP: i32 = 16;
const BUTTON_H: i32 = 15;
const LEFT_BUTTON_X: i32 = 90;
const RIGHT_X: i32 = 186;
const RIGHT_W: i32 = 94;
const PREVIEW_X: i32 = RIGHT_X + (RIGHT_W - 64) / 2;
/// The texture sample: SCREEN.CPS's top left corner, left of the MENTAT button.
const TEXTURE_W: usize = 16;
const TEXTURE_H: usize = 16;

/// The houses that can be opponents, in row order.
const OPPONENTS: [u8; 4] = [HOUSE_HARKONNEN, HOUSE_ATREIDES, HOUSE_ORDOS, HOUSE_SARDAUKAR];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Button {
    House,
    Opponent(u8),
    Seed,
    Size,
    Skill,
    Music,
    Sounds,
    NoFog,
    Start,
}

/// The windows' lettering: Atreides blue (whatever house is playing), like "Credits" on the interface.
pub(super) fn window_ink() -> u8 {
    palette_create_remap(HOUSE_ATREIDES)[147]
}

/// See `GameUi::window_art`.
pub struct WindowArt {
    texture: Vec<u8>,
    shade: [u8; 256],
}

pub struct SetupMenu {
    pub settings: SkirmishSettings,
    pub music: bool,
    pub sounds: bool,
    pub no_fog: bool,
    /// Set when START is clicked; the front-end then starts the game.
    pub done: bool,
    /// Shareware data: only Atreides can be played (only its voices exist).
    shareware: bool,
    /// The button held down with the left (false) or right (true) mouse button.
    pressed: Option<(Button, bool)>,
    /// The seed the preview was made for (and the seed it resolved to), so it is redone on change.
    preview_for: Option<(u32, u16, u8, u32)>,
    preview_seed: u32,
    preview: Vec<u8>,
    background: Vec<u8>,
    rng: u32,
}

impl SetupMenu {
    pub fn new(shareware: bool, seed: u32) -> SetupMenu {
        SetupMenu {
            settings: SkirmishSettings { player_house: HOUSE_ATREIDES, opponents: vec![HOUSE_HARKONNEN], map_seed: seed, ..Default::default() },
            music: true,
            sounds: true,
            no_fog: false,
            done: false,
            shareware,
            pressed: None,
            preview_for: None,
            preview_seed: seed,
            preview: Vec::new(),
            background: Vec::new(),
            rng: seed.wrapping_mul(2_654_435_761).wrapping_add(12345),
        }
    }

    /// The opponents as a bitmask of house ids.
    pub fn opponent_mask(&self) -> u32 {
        self.settings.opponents.iter().fold(0, |m, &h| m | 1 << h)
    }

    fn buttons(&self) -> Vec<(Button, i32, i32, i32)> {
        let mut b = vec![(Button::House, LEFT_BUTTON_X, ROW_Y, 88)];
        for (i, &h) in OPPONENTS.iter().enumerate() {
            b.push((Button::Opponent(h), LEFT_BUTTON_X, ROW_Y + ROW_STEP * (i as i32 + 1), 88));
        }
        b.push((Button::Seed, LEFT_BUTTON_X, ROW_Y + ROW_STEP * 5, 88));
        b.push((Button::Size, LEFT_BUTTON_X, ROW_Y + ROW_STEP * 6, 88));
        b.push((Button::Skill, LEFT_BUTTON_X, ROW_Y + ROW_STEP * 7, 88));
        b.push((Button::Music, RIGHT_X, ROW_Y + 80, RIGHT_W));
        b.push((Button::Sounds, RIGHT_X, ROW_Y + 80 + ROW_STEP, RIGHT_W));
        b.push((Button::NoFog, RIGHT_X, ROW_Y + 80 + ROW_STEP * 2, RIGHT_W));
        b.push((Button::Start, (WIN_W - 104) / 2, WIN_H - 22, 104));
        b
    }

    fn label(b: Button) -> &'static str {
        match b {
            Button::House => "Your house",
            Button::Opponent(h) => HOUSE_INFO[h as usize].name,
            Button::Seed => "Map seed",
            Button::Size => "Map size",
            Button::Skill => "Difficulty",
            _ => "",
        }
    }

    fn text(&self, b: Button) -> String {
        let on = |v: bool| if v { "ON" } else { "OFF" };
        match b {
            Button::House => HOUSE_INFO[self.settings.player_house as usize].name.to_string(),
            Button::Opponent(h) if h == self.settings.player_house => "You".into(),
            Button::Opponent(h) if self.settings.opponents.contains(&h) => "Enemy".into(),
            Button::Opponent(_) => "-".into(),
            Button::Seed => self.settings.map_seed.to_string(),
            Button::Size => if self.settings.map_scale == 0 { "Large 62x62" } else { "Small 32x32" }.into(),
            Button::Skill => match self.settings.difficulty {
                AiDifficulty::Easy => "Easy",
                AiDifficulty::Normal => "Normal",
                AiDifficulty::Hard => "Hard",
            }
            .into(),
            Button::Music => format!("Music {}", on(self.music)),
            Button::Sounds => format!("Sounds {}", on(self.sounds)),
            Button::NoFog => format!("Fog of war {}", on(!self.no_fog)),
            Button::Start => "START".into(),
        }
    }

    fn enabled(&self, b: Button) -> bool {
        match b {
            Button::House => !self.shareware,
            Button::Opponent(h) => h != self.settings.player_house,
            _ => true,
        }
    }

    /// A click on `b` (right click = `back`): cycle its value.
    fn activate(&mut self, b: Button, back: bool) {
        let s = &mut self.settings;
        match b {
            Button::House => {
                const HOUSES: [u8; 3] = [HOUSE_HARKONNEN, HOUSE_ATREIDES, HOUSE_ORDOS];
                let i = HOUSES.iter().position(|&h| h == s.player_house).unwrap_or(1);
                s.player_house = HOUSES[if back { i + 2 } else { i + 1 } % 3];
                s.opponents.retain(|&h| h != s.player_house);
                if s.opponents.is_empty() {
                    s.opponents.push(if s.player_house == HOUSE_HARKONNEN { HOUSE_ATREIDES } else { HOUSE_HARKONNEN });
                }
            }
            Button::Opponent(h) => {
                if let Some(i) = s.opponents.iter().position(|&o| o == h) {
                    // At least one opponent.
                    if s.opponents.len() > 1 {
                        s.opponents.remove(i);
                    }
                } else {
                    s.opponents.push(h);
                    s.opponents.sort_by_key(|o| OPPONENTS.iter().position(|x| x == o));
                }
            }
            // Clicking the seed rolls a random one (digits type one).
            Button::Seed => {
                self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
                s.map_seed = (self.rng >> 16) & 0x7FFF;
            }
            Button::Size => s.map_scale ^= 1,
            Button::Skill => {
                s.difficulty = match (s.difficulty, back) {
                    (AiDifficulty::Easy, false) | (AiDifficulty::Hard, true) => AiDifficulty::Normal,
                    (AiDifficulty::Normal, false) | (AiDifficulty::Easy, true) => AiDifficulty::Hard,
                    _ => AiDifficulty::Easy,
                }
            }
            Button::Music => self.music = !self.music,
            Button::Sounds => self.sounds = !self.sounds,
            Button::NoFog => self.no_fog = !self.no_fog,
            Button::Start => self.done = true,
        }
    }

    fn button_at(&self, x: i32, y: i32) -> Option<Button> {
        let (x, y) = (x - WIN_X, y - WIN_Y);
        self.buttons().into_iter().find(|&(b, bx, by, bw)| self.enabled(b) && x >= bx && x < bx + bw && y >= by && y < by + BUTTON_H).map(|(b, ..)| b)
    }
}

impl GameUi {
    /// Show the setup screen (`seed` = the initial map seed).
    pub fn setup_open(&mut self, shareware: bool, seed: u32) {
        self.setup = Some(SetupMenu::new(shareware, seed & 0x7FFF));
        self.options = None;
    }

    /// Back to the setup screen after a game, with the choices it was started with and the map
    /// it was played on. `start` = start that same game again straight away.
    pub fn setup_reopen(&mut self, start: bool) {
        let seed = self.game.scenario.map_seed;
        let shareware = self.assets.variant == crate::data::DataVariant::Shareware;
        let mut m = self.last_setup.take().unwrap_or_else(|| SetupMenu::new(shareware, seed));
        m.settings.map_seed = seed;
        m.done = start;
        m.pressed = None;
        m.preview_for = None;
        self.setup = Some(m);
        self.options = None;
        self.modal = None;
        self.factory = None;
        self.end = None;
        self.input.queue.clear();
    }

    /// One 60 Hz tick of the setup screen: handle clicks and keys.
    pub fn setup_tick(&mut self) {
        let Some(mut m) = self.setup.take() else { return };
        while let Some(k) = self.input.queue.pop_front() {
            let (x, y) = (self.input.click_x, self.input.click_y);
            match k {
                0x41 | 0x42 => m.pressed = m.button_at(x, y).map(|b| (b, k == 0x42)),
                0x841 | 0x842 => {
                    if let Some((b, back)) = m.pressed.take() {
                        if back == (k == 0x842) && m.button_at(x, y) == Some(b) {
                            m.activate(b, back);
                        }
                    }
                }
                // Digits type a seed, Backspace removes one, Enter starts.
                0x02..=0x0b => {
                    let digit = if k == 0x0b { 0 } else { k as u32 - 1 };
                    let seed = m.settings.map_seed * 10 + digit;
                    m.settings.map_seed = if seed > 0x7FFF { digit } else { seed };
                }
                0x0f => m.settings.map_seed /= 10,
                0x2b => m.done = true,
                _ => {}
            }
        }
        self.setup = Some(m);
    }

    pub fn setup_render(&mut self) -> &[u8] {
        let Some(mut m) = self.setup.take() else { return &self.screen.pixels };
        if m.background.is_empty() {
            m.background = self.setup_background();
        }
        let key = (m.settings.map_seed, m.settings.map_scale, m.settings.player_house, m.opponent_mask());
        if m.preview_for != Some(key) {
            m.preview_for = Some(key);
            self.setup_make_preview(&mut m);
        }
        self.screen.pixels.copy_from_slice(&m.background);
        self.draw_sand_window(WIN_X, WIN_Y, WIN_W, WIN_H);

        let ink = window_ink();
        let title: &[u8] = if m.shareware { b"Dune II Shareware - Skirmish" } else { b"Dune II - Skirmish" };
        self.engraved_text(title, WIN_X + WIN_W / 2, WIN_Y + 7, ink, 1);

        let hover = m.button_at(self.input.mouse_x, self.input.mouse_y);
        let held = self.input.left_down || self.input.right_down;
        for (b, bx, by, bw) in m.buttons() {
            let (x, y) = (WIN_X + bx, WIN_Y + by);
            let label = SetupMenu::label(b);
            if !label.is_empty() {
                self.engraved_text(label.as_bytes(), x - 8, y + 3, ink, 2);
            }
            // Buttons like MENTAT and OPTIONS: a dark outline around a bevelled gold face.
            let down = held && m.pressed.map(|p| p.0) == Some(b) && hover == Some(b);
            self.gold_button(x, y, bw, BUTTON_H, down);
            let d = down as i32;
            // Disabled: in the face's shadow colour.
            let colour = if m.enabled(b) { ink } else { 226 };
            let t = m.text(b);
            if b == Button::Start {
                self.engraved_text(t.as_bytes(), x + bw / 2 + d, y + 2 + d, colour, 1);
            } else {
                self.engraved_text(t.as_bytes(), x + 4 + d, y + 2 + d, colour, 0);
            }
        }

        // The map preview, in a recessed frame.
        let (px, py) = (WIN_X + PREVIEW_X, WIN_Y + ROW_Y);
        self.screen.wired_rect(px - 2, py - 2, px + 65, py + 65, 229);
        self.bevel(px - 3, py - 3, 70, 70, 226, 108);
        for y in 0..64 {
            let row = (py + y) as usize * SCREEN_WIDTH + px as usize;
            self.screen.pixels[row..row + 64].copy_from_slice(&m.preview[y as usize * 64..y as usize * 64 + 64]);
        }
        let caption = format!("Map {}", m.preview_seed);
        self.engraved_text(caption.as_bytes(), px + 32, py + 69, ink, 1);

        self.setup = Some(m);
        let s = self.sprite(0).cloned();
        self.screen.draw_sprite(s.as_ref(), self.input.mouse_x, self.input.mouse_y, 0, 0, None, None);
        &self.screen.pixels
    }

    /// Top/left lines in `light`, bottom/right lines in `dark`.
    pub(super) fn bevel(&mut self, x: i32, y: i32, w: i32, h: i32, light: u8, dark: u8) {
        self.screen.line(x, y + h - 1, x + w - 1, y + h - 1, dark);
        self.screen.line(x + w - 1, y, x + w - 1, y + h - 1, dark);
        self.screen.line(x, y, x + w - 2, y, light);
        self.screen.line(x, y, x, y + h - 2, light);
    }

    /// A button drawn like the interface's MENTAT and OPTIONS buttons (SCREEN.CPS): outline 229,
    /// highlight 108, shadow 226 and face 115. Pressed, the bevel is reversed.
    pub(super) fn gold_button(&mut self, x: i32, y: i32, w: i32, h: i32, pressed: bool) {
        self.screen.fill_rect(x + 1, y + 1, x + w - 2, y + h - 2, 115);
        self.screen.wired_rect(x, y, x + w - 1, y + h - 1, 229);
        let (light, dark) = if pressed { (226, 108) } else { (108, 226) };
        self.bevel(x + 1, y + 1, w - 2, h - 2, light, dark);
    }

    /// Bold text cut into the gold like the interface's lettering: a light edge below and to the
    /// right. `align`: 0 = left, 1 = centred on `x`, 2 = right-aligned to `x`.
    pub(super) fn engraved_text(&mut self, s: &[u8], x: i32, y: i32, colour: u8, align: i32) {
        // The 8p font with one pixel of spacing (style 0x20), its outline colours made clear.
        self.screen.draw_text_wrapper(&self.fonts, b"", 0, 0, 0, 0, 0x22);
        self.screen.colours[2] = 0;
        self.screen.colours[3] = 0;
        // Bold: every stroke drawn twice, a pixel apart.
        let width = self.screen.string_width(&self.fonts, s) + 1;
        let x = x - width * align / 2;
        for (dx, dy, c) in [(1, 1, 108), (2, 1, 108), (0, 0, colour), (1, 0, colour)] {
            self.screen.draw_text(&self.fonts, s, x + dx, y + dy, c, 0);
        }
    }

    /// The art the setup and options windows are made of, loaded once: the sand texture and
    /// ShadeScreen's half-brightness colour map.
    fn window_art(&mut self) -> &WindowArt {
        if self.window_art.is_none() {
            let mut texture = vec![115; TEXTURE_W * TEXTURE_H];
            if let Some(cps) = self.assets.vfs.get("SCREEN.CPS") {
                let pixels = crate::data::image::load_cps(cps).pixels;
                texture = (0..TEXTURE_H).flat_map(|y| pixels[y * SCREEN_WIDTH..y * SCREEN_WIDTH + TEXTURE_W].to_vec()).collect();
            }
            let p = &self.palette;
            let mut shade = [0u8; 256];
            for (c, out) in shade.iter_mut().enumerate() {
                let target = [p[c * 3] as i32 / 2, p[c * 3 + 1] as i32 / 2, p[c * 3 + 2] as i32 / 2];
                *out = (0..256usize)
                    .min_by_key(|&i| (0..3).map(|k| (p[i * 3 + k] as i32 - target[k]).pow(2)).sum::<i32>())
                    .unwrap_or(0) as u8;
            }
            self.window_art = Some(WindowArt { texture, shade });
        }
        self.window_art.as_ref().unwrap()
    }

    /// ShadeScreen: everything on screen at half brightness, as behind the original's option windows.
    pub(super) fn shade_screen(&mut self) {
        let shade = self.window_art().shade;
        for px in self.screen.pixels.iter_mut() {
            *px = shade[*px as usize];
        }
    }

    /// A window of the marbled sand of the interface's top strip (each 16x16 tile of it mirrored
    /// at random so it repeats without an obvious pattern), in a dark outline with a light and dark
    /// bevel.
    pub(super) fn draw_sand_window(&mut self, left: i32, top: i32, width: i32, height: i32) {
        let texture = self.window_art().texture.clone();
        for y in 0..height as usize {
            for x in 0..width as usize {
                let tile = ((x / TEXTURE_W) as u32).wrapping_mul(0x9E37_79B9) ^ ((y / TEXTURE_H) as u32).wrapping_mul(0x85EB_CA6B);
                let tile = tile ^ (tile >> 15);
                let (tx, ty) = (x % TEXTURE_W, y % TEXTURE_H);
                let tx = if tile & 1 != 0 { TEXTURE_W - 1 - tx } else { tx };
                let ty = if tile & 2 != 0 { TEXTURE_H - 1 - ty } else { ty };
                self.screen.put_pixel(left + x as i32, top + y as i32, texture[ty * TEXTURE_W + tx]);
            }
        }
        self.screen.wired_rect(left, top, left + width - 1, top + height - 1, 229);
        for (i, (light, dark)) in [(105, 227), (108, 226)].into_iter().enumerate() {
            let i = i as i32 + 1;
            self.bevel(left + i, top + i, width - 2 * i, height - 2 * i, light, dark);
        }
    }

    /// The interface (SCREEN.CPS) shaded, as behind an options window.
    fn setup_background(&mut self) -> Vec<u8> {
        let saved = std::mem::take(&mut self.screen.pixels);
        self.screen.pixels = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
        if let Some(cps) = self.assets.vfs.get("SCREEN.CPS") {
            self.screen.pixels = crate::data::image::load_cps(cps).pixels;
        }
        let credits = self.sprite(11).cloned();
        self.screen.draw_sprite(credits.as_ref(), 192, 0, 0, 0, None, None);
        self.screen.remap_area(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, &palette_create_remap(HOUSE_ATREIDES));
        self.shade_screen();
        std::mem::replace(&mut self.screen.pixels, saved)
    }

    /// Lay out the skirmish as it would start and draw its whole map in radar colours (64x64).
    fn setup_make_preview(&mut self, m: &mut SetupMenu) {
        let GameUi { game, assets, .. } = self;
        game.start_skirmish(assets, &m.settings);
        game.events.clear();
        let g = &self.game;
        m.preview_seed = g.scenario.map_seed;
        m.preview = vec![12; 64 * 64];
        let (min_x, min_y, width, _) = g.map_info();
        let scale = (64 / width.max(1)).max(1) as usize;
        for packed in 0..4096u16 {
            if !g.map_is_valid_position(packed) {
                continue;
            }
            let t = g.map_get_landscape_type(packed) as usize;
            let mut colour = LANDSCAPE_INFO[t].radar_colour;
            if colour == 0xFFFF {
                colour = HOUSE_INFO[g.map[packed as usize].house_id as usize].minimap_color;
            }
            if let Some(u) = g.unit_get_by_packed_tile(packed) {
                colour = HOUSE_INFO[g.unit_get_house_id(u) as usize].minimap_color;
            }
            let x = (packed_x(packed) - min_x) as usize * scale;
            let y = (packed_y(packed) - min_y) as usize * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    if x + dx < 64 && y + dy < 64 {
                        m.preview[(y + dy) * 64 + x + dx] = colour as u8;
                    }
                }
            }
        }
    }
}
