//! The factory / construction yard / starport window (GUI_DisplayFactoryWindow).

use super::screen::*;
use super::widget::*;
use super::GameUi;
use crate::data::wsa::Wsa;
use crate::game::structure::FactoryResult;
use crate::game::*;
use crate::tables::strings::*;
use crate::tables::{ObjectInfo, STRUCTURE_LAYOUT_SIZE};

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub object_type: u16,
    pub amount: i8,
    pub credits: u16,
    pub sort_priority: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FactoryClick {
    List(u16),
    Up,
    Down,
    Resume,
    Upgrade,
    Plus,
    Minus,
    Invoice,
    BuildThis,
}

pub struct FactoryWindow {
    structure: u16,
    construction_yard: bool,
    starport: bool,
    upgrade_cost: u16,
    items: Vec<Item>,
    selected: usize,
    base: usize,
    ordered: u16,
    background: Vec<u8>,
    widgets: Widgets,
    clicks: Vec<FactoryClick>,
    graymap: [u8; 256],
    preview: Option<(usize, usize, Vec<u8>)>,
    preview_for: Option<usize>,
    invoice: bool,
    result: Option<FactoryResult>,
    pal_timer: u32,
    pal_colour: i8,
    pal_change: i8,
    pal_backup: [u8; 3],
}

impl FactoryWindow {
    pub fn new(ui: &mut GameUi, structure: u16, construction_yard: bool, starport: bool, upgrade_cost: u16) -> FactoryWindow {
        let mut graymap = [0u8; 256];
        if let Some(t) = ui.assets.vfs.get("GRAYRMAP.TBL") {
            let n = t.len().min(256);
            graymap[..n].copy_from_slice(&t[..n]);
        } else {
            for (i, g) in graymap.iter_mut().enumerate() {
                *g = i as u8;
            }
        }
        // Background: CHOAM.CPS, "Credits", house remap and the house-coloured side panels.
        let mut s = Screen::new();
        if let Some(cps) = ui.assets.vfs.get("CHOAM.CPS") {
            s.pixels = crate::data::image::load_cps(cps).pixels;
        } else {
            // Shareware data has no CHOAM.CPS: a plain frame with the same layout.
            s.fill_rect(0, 0, 319, 199, 116);
            s.border(64, 16, 48, 136, 2, true); // item list
            s.border(120, 16, 196, 24, 2, true); // caption
            s.border(120, 44, 196, 120, 2, true); // preview
        }
        let credits = ui.sprite(11).cloned();
        s.draw_sprite(credits.as_ref(), 192, 0, 0, 0, None, None);
        s.remap_area(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, &ui.remap);
        const XSRC: [usize; 6] = [0, 0, 16, 0, 0, 0];
        const YSRC: [usize; 6] = [8, 152, 48, 0, 0, 0];
        let h = ui.game.player_house_id as usize;
        let copy = |s: &mut Screen, sx: usize, sy: usize, dx: usize, dy: usize, w: usize, hgt: usize| {
            let src = s.pixels.clone();
            for row in 0..hgt {
                for col in 0..w {
                    let (a, b) = ((sy + row) * SCREEN_WIDTH + sx + col, (dy + row) * SCREEN_WIDTH + dx + col);
                    if a < src.len() && b < s.pixels.len() {
                        s.pixels[b] = src[a];
                    }
                }
            }
        };
        copy(&mut s, XSRC[h] * 8, YSRC[h], 0, 8, 56, 40);
        copy(&mut s, XSRC[h] * 8, YSRC[h], 0, 152, 56, 40);

        let mut fw = FactoryWindow {
            structure,
            construction_yard,
            starport,
            upgrade_cost,
            items: Vec::new(),
            selected: 0,
            base: 0,
            ordered: 0,
            background: s.pixels,
            widgets: Widgets::default(),
            clicks: Vec::new(),
            graymap,
            preview: None,
            preview_for: None,
            invoice: false,
            result: None,
            pal_timer: 0,
            pal_colour: 0,
            pal_change: 8,
            pal_backup: [ui.palette[765], ui.palette[766], ui.palette[767]],
        };
        fw.create_widgets(ui);
        fw.init_items(ui);
        for i in fw.items.len()..4 {
            fw.widgets.make_visible(i as u16 + 46, false);
        }
        fw.update_selection(ui, true);
        fw
    }

    /// GUI_FactoryWindow_CreateWidgets
    fn create_widgets(&mut self, ui: &GameUi) {
        // (clickProc, shortcut, flags, spriteID, offsetX, offsetY, width, height)
        let defs: [(FactoryClick, i32, u16, i32, i16, i16, u16, u16); 13] = [
            (FactoryClick::List(0), -2, 4544, -1, 72, 24, 32, 24),
            (FactoryClick::List(1), -3, 4544, -1, 72, 56, 32, 24),
            (FactoryClick::List(2), -4, 4544, -1, 72, 88, 32, 24),
            (FactoryClick::List(3), -5, 4544, -1, 72, 120, 32, 24),
            (FactoryClick::Up, -1120, 4548, 355, 64, 168, 48, 16),
            (FactoryClick::Down, -1122, 4548, 357, 64, 184, 48, 16),
            (FactoryClick::Resume, 188, 17607, 365, 160, 184, 120, 16),
            (FactoryClick::Upgrade, 189, 17607, 359, 112, 1, 88, 16),
            (FactoryClick::Plus, -13, 17607, 367, 8, 80, 40, 16),
            (FactoryClick::Minus, -12, 17607, 369, 8, 104, 40, 16),
            (FactoryClick::Invoice, 192, 17607, 371, 112, 1, 88, 16),
            (FactoryClick::BuildThis, 191, 17607, 363, 160, 168, 120, 16),
            (FactoryClick::BuildThis, 190, 17607, 361, 160, 168, 120, 16),
        ];
        for (i, &(click, shortcut, flags, sprite, ox, oy, w, h)) in defs.iter().enumerate() {
            if (i == 8 || i == 9 || i == 10 || i == 12) && !self.starport {
                continue;
            }
            if i == 11 && self.starport {
                continue;
            }
            if i == 7 && self.upgrade_cost == 0 {
                continue;
            }
            let sc = if shortcut < 0 { (-shortcut) as u16 } else { get_shortcut(*ui.game.strings.get(shortcut as u16).first().unwrap_or(&0)) };
            let kind = if sprite < 0 { WidgetKind::None } else { WidgetKind::Sprite { normal: sprite as u16, down: sprite as u16 + 1 } };
            self.widgets.list.push(Widget {
                index: i as u16 + 46,
                shortcut: sc,
                shortcut2: sc,
                kind,
                click: ClickProc::None,
                string_id: STR_NULL,
                requires_click: flags & 1 != 0,
                click_as_hover: flags & 4 != 0,
                invisible: flags & 8 != 0,
                grey_when_invisible: flags & 0x10 != 0,
                no_click_cascade: flags & 0x20 != 0,
                lose_select: flags & 0x40 != 0,
                button_filter_left: (flags >> 8) & 0xF,
                button_filter_right: (flags >> 12) & 0xF,
                parent_id: 0,
                offset_x: ox,
                offset_y: oy,
                width: w,
                height: h,
                selected: false,
                hover1: false,
                hover2: false,
                selected_last: false,
                hover1_last: false,
                key_selected: false,
                button_state: 0,
            });
            self.clicks.push(click);
        }
        self.widgets.reset = true;
    }

    fn info<'a>(&self, ui: &'a GameUi, item: &Item) -> &'a ObjectInfo {
        if self.construction_yard {
            &ui.game.structure_info[item.object_type as usize].o
        } else {
            &ui.game.unit_info[item.object_type as usize].o
        }
    }

    /// GUI_FactoryWindow_InitItems
    fn init_items(&mut self, ui: &mut GameUi) {
        let g = &mut ui.game;
        if self.starport {
            let seconds = (g.timer_game.wrapping_sub(g.tick_scenario_start) / 60) as u16;
            let mut seed = (seconds / 60).wrapping_add(g.scenario_id).wrapping_add(g.player_house_id as u16);
            seed = seed.wrapping_mul(seed);
            g.rng.seed_lcg(seed);
        }
        if !self.construction_yard {
            for i in 0..UNIT_MAX {
                let oi = g.unit_info[i].o;
                if oi.available == 0 {
                    continue;
                }
                let credits = if self.starport {
                    let c = oi.build_credits / 10;
                    let r = g.rng.lcg_range(0, 6) + g.rng.lcg_range(0, 6);
                    (c * 4 + c * r).min(999)
                } else {
                    oi.build_credits
                };
                self.items.push(Item { object_type: i as u16, amount: 0, credits, sort_priority: oi.sort_priority as u16 });
            }
        } else {
            for i in 0..STRUCTURE_MAX {
                let oi = g.structure_info[i].o;
                if oi.available == 0 {
                    continue;
                }
                let sp = if i == 0 || i == 1 { 0x64 } else { oi.sort_priority as u16 };
                self.items.push(Item { object_type: i as u16, amount: 0, credits: oi.build_credits, sort_priority: sp });
            }
        }
        self.items.sort_by(|a, b| b.sort_priority.cmp(&a.sort_priority));
    }

    fn item(&self, offset: i32) -> Option<usize> {
        let i = offset + self.base as i32;
        (i >= 0 && (i as usize) < self.items.len()).then_some(i as usize)
    }

    /// GUI_FactoryWindow_UpdateSelection: the pulsing selection colour (palette 255).
    fn update_selection(&mut self, ui: &mut GameUi, changed: bool) {
        if changed {
            ui.palette[765] = 0x3F;
            ui.palette[766] = 0x3F;
            ui.palette[767] = 0x3F;
            self.pal_timer = 0;
            self.pal_colour = 0;
            self.pal_change = 8;
        } else if self.pal_timer > ui.game.timer_gui {
            return;
        }
        self.pal_timer = ui.game.timer_gui + 3;
        self.pal_colour += self.pal_change;
        if self.pal_colour < 0 || self.pal_colour > 63 {
            self.pal_change = -self.pal_change;
            self.pal_colour += self.pal_change;
            return;
        }
        let c = self.pal_colour as u8;
        match ui.game.player_house_id {
            HOUSE_HARKONNEN => {
                ui.palette[766] = c;
                ui.palette[767] = c;
            }
            HOUSE_ATREIDES => {
                ui.palette[765] = c;
                ui.palette[766] = c;
            }
            HOUSE_ORDOS => {
                ui.palette[765] = c;
                ui.palette[767] = c;
            }
            _ => {}
        }
    }

    /// One tick of the factory window's event loop.
    pub fn tick(&mut self, ui: &mut GameUi) {
        self.update_selection(ui, false);
        let mut widgets = std::mem::take(&mut self.widgets);
        let mut input = std::mem::take(&mut ui.input);
        let mut clicked: Vec<usize> = Vec::new();
        let key = widgets.handle_events(&mut input, &mut |_, i| {
            clicked.push(i);
            true
        });
        self.widgets = widgets;
        ui.input = input;
        if key == 0x6E {
            self.click(ui, FactoryClick::Resume);
        }
        for i in clicked {
            let c = self.clicks[i];
            if self.invoice && c != FactoryClick::Invoice && c != FactoryClick::BuildThis && c != FactoryClick::Resume {
                self.invoice = false;
                continue;
            }
            self.click(ui, c);
            for w in self.widgets.list.iter_mut() {
                w.selected = false;
            }
        }
    }

    fn click(&mut self, ui: &mut GameUi, c: FactoryClick) {
        let ph = ui.game.player_house_id;
        match c {
            FactoryClick::List(i) => {
                if (i as usize) < self.items.len().min(4) {
                    self.selected = i as usize;
                    self.update_selection(ui, true);
                }
            }
            FactoryClick::Down => {
                if self.selected < 3 && self.selected + 1 < self.items.len() {
                    self.selected += 1;
                } else if self.base + 4 < self.items.len() {
                    self.base += 1;
                }
                self.update_selection(ui, true);
            }
            FactoryClick::Up => {
                if self.selected != 0 {
                    self.selected -= 1;
                } else if self.base != 0 {
                    self.base -= 1;
                }
                self.update_selection(ui, true);
            }
            FactoryClick::Resume => {
                if self.starport {
                    for it in self.items.iter_mut() {
                        if it.amount != 0 {
                            let h = ui.game.hm(ph);
                            h.credits = h.credits.wrapping_add(it.amount as u16 * it.credits);
                            it.amount = 0;
                        }
                    }
                    self.ordered = 0;
                }
                self.result = Some(FactoryResult::Resume);
            }
            FactoryClick::Upgrade => self.result = Some(FactoryResult::Upgrade),
            FactoryClick::Invoice => self.invoice = !self.invoice,
            FactoryClick::BuildThis => {
                if self.starport {
                    if self.ordered == 0 {
                        self.invoice = true;
                    } else {
                        let list = self.items.iter().map(|it| (it.object_type, it.amount as u16)).collect();
                        self.result = Some(FactoryResult::Buy(list));
                    }
                } else if let Some(i) = self.item(self.selected as i32) {
                    if self.info(ui, &self.items[i]).available > 0 {
                        self.items[i].amount = 1;
                        let list = self.items.iter().map(|it| (it.object_type, it.amount as u16)).collect();
                        self.result = Some(FactoryResult::Buy(list));
                    }
                }
            }
            FactoryClick::Plus => {
                let Some(i) = self.item(self.selected as i32) else { return };
                let it = self.items[i];
                let available = self.info(ui, &it).available;
                if (it.amount as i32) < available as i32 && it.credits <= ui.game.h(ph).credits {
                    self.items[i].amount += 1;
                    self.ordered += 1;
                    ui.game.hm(ph).credits -= it.credits;
                }
            }
            FactoryClick::Minus => {
                let Some(i) = self.item(self.selected as i32) else { return };
                if self.items[i].amount != 0 {
                    self.items[i].amount -= 1;
                    self.ordered -= 1;
                    let c = self.items[i].credits;
                    let h = ui.game.hm(ph);
                    h.credits = h.credits.wrapping_add(c);
                }
            }
        }
    }

    /// The window result, if it has been closed.
    pub fn take_result(&mut self, ui: &mut GameUi) -> Option<(u16, FactoryResult)> {
        let r = self.result.take()?;
        ui.palette[765] = self.pal_backup[0];
        ui.palette[766] = self.pal_backup[1];
        ui.palette[767] = self.pal_backup[2];
        ui.game.player_credits = 0xFFFF;
        Some((self.structure, r))
    }

    pub fn render(&mut self, ui: &mut GameUi) {
        ui.screen.pixels.copy_from_slice(&self.background);
        let fonts = &ui.fonts;
        // Item icons
        for i in 0..4 {
            let Some(idx) = self.item(i) else { continue };
            let it = self.items[idx];
            let oi = *self.info(ui, &it);
            let s = ui.sprite(oi.sprite_id).cloned();
            if oi.available == -1 {
                ui.screen.draw_sprite(s.as_ref(), 72, 24 + i * 32, 0, DRAWSPRITE_FLAG_REMAP, None, Some((&self.graymap, 1)));
            } else {
                ui.screen.draw_sprite(s.as_ref(), 72, 24 + i * 32, 0, 0, None, None);
            }
        }
        // Selection rectangle
        let y = self.selected as i32 * 32 + 24;
        ui.screen.wired_rect(71, y - 1, 104, y + 23 + 1, 255);
        ui.screen.wired_rect(72, y, 103, y + 23, 255);

        let Some(sel) = self.item(self.selected as i32) else { return };
        let item = self.items[sel];
        let oi = *self.info(ui, &item);
        if self.invoice {
            self.render_invoice(ui);
        } else {
            // Preview picture (WSA frame 0)
            if self.preview_for != Some(sel) {
                self.preview_for = Some(sel);
                self.preview = ui.assets.vfs.get(oi.wsa).and_then(Wsa::load).and_then(|w| w.frames.first().map(|f| (w.width, w.height, f.clone())));
            }
            if self.preview.is_none() {
                // No WSA (shareware data): a plain sand backdrop with the item's icon at 2x.
                ui.screen.fill_rect(128, 48, 128 + 184 - 1, 48 + 112 - 1, 34);
                let icon = ui.sprite(oi.sprite_id).cloned();
                if let Some(icon) = icon {
                    let (w, h) = (icon.width as i32, icon.height as i32);
                    let mut tmp = Screen::new();
                    tmp.draw_sprite(Some(&icon), 0, 0, 0, 0, None, None);
                    let (ox, oy) = (128 + (184 - w * 2) / 2, 48 + (112 - h * 2) / 2);
                    for y in 0..h * 2 {
                        for x in 0..w * 2 {
                            let c = tmp.pixels[(y / 2) as usize * SCREEN_WIDTH + (x / 2) as usize];
                            if c != 0 {
                                ui.screen.pixels[(oy + y) as usize * SCREEN_WIDTH + (ox + x) as usize] = c;
                            }
                        }
                    }
                }
            }
            if let Some((w, h, px)) = &self.preview {
                for row in 0..*h {
                    for col in 0..*w {
                        let (x, y) = (128 + col, 48 + row);
                        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
                            ui.screen.pixels[y * SCREEN_WIDTH + x] = px[row * w + col];
                        }
                    }
                }
            }
            if self.construction_yard {
                let s64 = ui.sprite(64).cloned();
                ui.screen.draw_sprite(s64.as_ref(), 288, 136, 0, 0, None, None);
                let s24 = ui.sprite(24).cloned();
                let sw = s24.as_ref().map(|s| s.width as i32).unwrap_or(0) + 1;
                let si = ui.game.structure_info[item.object_type as usize];
                let (lw, lh) = STRUCTURE_LAYOUT_SIZE[si.layout as usize];
                for j in 0..lh as i32 {
                    for i in 0..lw as i32 {
                        ui.screen.draw_sprite(s24.as_ref(), 289 + i * sw, 137 + j * sw, 0, 0, None, None);
                    }
                }
            }
            if oi.available == -1 {
                ui.screen.remap_area(128, 48, 184, 112, &self.graymap);
                if self.starport {
                    let t = ui.game.string(STR_OUT_OF_STOCK);
                    ui.screen.draw_text_wrapper(fonts, &t, 220, 99, 6, 0, 0x132);
                } else {
                    let t = ui.game.string(STR_NEED_STRUCTURE_UPGRADE);
                    ui.screen.draw_text_wrapper(fonts, &t, 220, 94, 6, 0, 0x132);
                    let t = if self.upgrade_cost != 0 {
                        format_c(&ui.game.string(STR_UPGRADE_COST_D), &[Arg::D(self.upgrade_cost as i32)])
                    } else {
                        ui.game.string(STR_REPAIR_STRUCTURE_FIRST)
                    };
                    ui.screen.draw_text_wrapper(fonts, &t, 220, 104, 6, 0, 0x132);
                }
            } else if self.starport && item.amount as i32 >= oi.available as i32 {
                let t = ui.game.string(STR_OUT_OF_STOCK);
                ui.screen.draw_text_wrapper(fonts, &t, 220, 99, 6, 0, 0x132);
            }
            // Caption
            ui.screen.fill_rect(128, 21, 310, 35, 116);
            let name = ui.game.string(oi.string_id_full);
            ui.screen.draw_text_wrapper(fonts, &name, 128, 23, 12, 0, 0x12);
            let mut width = ui.screen.string_width(fonts, &ui.game.string(STR_COST_999));
            let cost = format_c(&ui.game.string(STR_COST_3D), &[Arg::D(item.credits as i32)]);
            ui.screen.draw_text_wrapper(fonts, &cost, 310 - width, 23, 12, 0, 0x12);
            if self.starport {
                width += ui.screen.string_width(fonts, &ui.game.string(STR_QTY_99)) + 2;
                let q = format_c(&ui.game.string(STR_QTY_2D), &[Arg::D(item.amount as i32)]);
                ui.screen.draw_text_wrapper(fonts, &q, 310 - width, 23, 12, 0, 0x12);
            }
        }
        // Buttons
        for w in self.widgets.list.clone() {
            if w.invisible {
                continue;
            }
            if let WidgetKind::Sprite { normal, down } = w.kind {
                let s = ui.sprite(if w.hover2 { down } else { normal }).cloned();
                if s.is_none() {
                    // Shareware data has no CHOAMSHP.SHP: draw a plain labelled button instead.
                    let label = match w.index - 46 {
                        4 => b"Up".to_vec(),
                        5 => b"Down".to_vec(),
                        6 => ui.game.string(188),
                        7 => ui.game.string(189),
                        8 => b"+".to_vec(),
                        9 => b"-".to_vec(),
                        10 => ui.game.string(192),
                        11 => ui.game.string(191),
                        12 => ui.game.string(190),
                        _ => Vec::new(),
                    };
                    let (x, y, bw, bh) = (w.offset_x as i32, w.offset_y as i32, w.width as i32, w.height as i32);
                    ui.screen.border(x, y, bw, bh, if w.hover2 { 2 } else { 1 }, true);
                    let tw = ui.screen.string_width(&ui.fonts, &label);
                    let fonts = &ui.fonts;
                    ui.screen.draw_text_wrapper(fonts, &label, x + (bw - tw) / 2, y + (bh - 8) / 2 + 1, 15, 0, 0x12);
                    continue;
                }
                let remap = ui.remap;
                ui.screen.draw_sprite(s.as_ref(), w.offset_x as i32, w.offset_y as i32, 0, DRAWSPRITE_FLAG_REMAP | DRAWSPRITE_FLAG_WIDGETPOS, None, Some((&remap, 1)));
            }
        }
        ui.draw_credits();
    }

    fn render_invoice(&self, ui: &mut GameUi) {
        let fonts = &ui.fonts;
        ui.screen.fill_rect(128, 48, 311, 159, 20);
        let mut y = 48;
        let t = ui.game.string(STR_ITEM_NAME_QTY_TOTAL);
        ui.screen.draw_text_wrapper(fonts, &t, 128, y, 12, 0, 0x11);
        y += 7;
        ui.screen.line(129, y, 310, y, 12);
        y += 2;
        let mut total = 0i32;
        if self.ordered != 0 {
            for it in &self.items {
                if it.amount == 0 {
                    continue;
                }
                let amount = it.amount as i32 * it.credits as i32;
                total += amount;
                let oi = self.info(ui, it);
                let name = ui.game.string(oi.string_id_full);
                ui.screen.draw_text_wrapper(fonts, &name, 128, y, 8, 0, 0x11);
                let s = format!("{:02} {:5}", it.amount, amount).into_bytes();
                ui.screen.draw_text_wrapper(fonts, &s, 311 - s.len() as i32 * 6, y, 15, 0, 0x11);
                y += 8;
            }
        } else {
            let t = ui.game.string(STR_NO_UNITS_ON_ORDER);
            ui.screen.draw_text_wrapper(fonts, &t, 220, 99, 6, 0, 0x112);
        }
        ui.screen.line(129, 148, 310, 148, 12);
        ui.screen.line(129, 150, 310, 150, 12);
        let ts = total.to_string().into_bytes();
        let x = 311 - ts.len() as i32 * 6;
        let t = ui.game.string(STR_TOTAL_COST_);
        ui.screen.draw_text_wrapper(fonts, &t, x - 3, 152, 11, 0, 0x211);
        ui.screen.draw_text_wrapper(fonts, &ts, x, 152, 11, 0, 0x11);
        ui.screen.fill_rect(128, 21, 310, 35, 116);
        let t = ui.game.string(STR_INVOICE_OF_UNITS_ON_ORDER);
        ui.screen.draw_text_wrapper(fonts, &t, 128, 23, 12, 0, 0x12);
    }
}

impl GameUi {
    pub(crate) fn factory_tick(&mut self) {
        let Some(mut f) = self.factory.take() else { return };
        f.tick(self);
        self.credits_tick();
        if let Some((s, res)) = f.take_result(self) {
            self.game.structure_factory_result(s, res);
            self.process_events();
            return;
        }
        self.factory = Some(f);
    }
}
