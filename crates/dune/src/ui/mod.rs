//! The in-game user interface (gui.c, viewport.c, widget_draw.c, widget_click.c).
//!
//! The screen is fully redrawn every frame instead of the original's dirty-rectangle
//! bookkeeping; what is drawn and how it reacts to input follows the original.

pub mod factory;
pub mod options;
pub mod screen;
pub mod setup;
pub mod sound;
pub mod widget;

use crate::data::image::Sprite;
use crate::data::Assets;
use crate::game::map::*;
use crate::game::*;
use crate::tables::landscape::LANDSCAPE_INFO;
use crate::tables::strings::*;
use crate::tables::{ACTIONS_AI, ACTION_INFO, HOUSE_INFO, STRUCTURE_LAYOUT_SIZE, TILE_DIFF};
use crate::tile::{pack_xy, packed_x, packed_y, Tile32};
use screen::*;
pub use sound::SoundRequest;
use widget::*;

/// Optional quality-of-life features (all off = original behaviour).
#[derive(Clone, Debug, Default)]
pub struct QolSettings {
    /// Right click deselects / cancels (original: right click does nothing in the viewport).
    pub right_click_cancel: bool,
    /// Right click on the map gives the selected unit an order (attack / move / harvest / enter),
    /// like later RTS games. Implies `right_click_cancel` when there is nothing to order.
    pub right_click_orders: bool,
    /// Hide the first-time hint dialogs (the original shows them).
    pub no_hints: bool,
    /// The far right screen edge (beside the viewport) also scrolls right. The original's
    /// right scroll strip is x 240..249, between the viewport and the sidebar, which is hard to
    /// hit with a browser mouse that is not confined to the screen.
    pub screen_edge_scroll: bool,
}

/// GUI_DisplayText's static state.
#[derive(Default)]
struct TextDisplay {
    display_timer: u32,
    text_offset: u16,
    scroll_in_progress: bool,
    lines: [Vec<u8>; 3],
    importance: [i16; 3],
    colours: [u8; 3],
    needs_update: bool,
    strip: Vec<u8>,
}

/// A modal message box (GUI_DisplayModalMessage), shown on top of a frozen game.
pub struct Modal {
    text: Vec<u8>,
    sprite: u16,
    wait: u16,
}


pub struct GameUi {
    pub game: Game,
    pub assets: Assets,
    pub screen: Screen,
    text_screen: Screen,
    pub fonts: Fonts,
    pub widgets: Widgets,
    pub input: Input,
    pub palette: [u8; 768],
    remap: [u8; 256],
    pal_map1: [u8; 256],
    pal_map2: [u8; 256],
    interface: Vec<u8>,
    pub cursor_sprite: u16,
    cursor_default: u16,
    cursor_hotspot: (i32, i32),
    tick_cursor: u32,
    tick_map_scroll: u32,
    tick_click: u32,
    var_37b8: bool,
    text: TextDisplay,
    production_string_id: u16,
    structure_high_health: bool,
    credits_animation: u16,
    credits_animation_offset: i16,
    tick_credits: u32,
    pal_timer_animation: u32,
    pal_timer_selection: u32,
    pal_timer_toggle: u32,
    pal_animation_toggle: bool,
    pal_selection_colour: u16,
    pal_toggle_colour: u16,
    hints_shown: u64,
    pub modal: Option<Modal>,
    pub factory: Option<factory::FactoryWindow>,
    pub sounds: Vec<SoundRequest>,
    /// The audio engine, once the front-end has created it (`audio_init`).
    pub audio: Option<crate::audio::Audio>,
    music: sound::MusicState,
    pub qol: QolSettings,
    /// The Options button's "Game controls" window, while it is open (the game is paused).
    pub options: Option<options::OptionsWindow>,
    pub auto_scroll: bool,
    /// Turbo: simulation ticks per real tick, in percent (100 = original pace). Applies on top of
    /// the original game speed setting, which only affects building, movement and turning.
    pub sim_percent: u32,
    /// DEBUG ("No fog of war"): draw the minimap as if the player's radar were built and powered.
    /// Display only: the house's radar state (and its voice/feedback) is untouched.
    pub debug_full_radar: bool,
    sim_acc: u32,
    /// The skirmish setup screen, while it is shown.
    pub setup: Option<setup::SetupMenu>,
    window_art: Option<setup::WindowArt>,
}

const WIDGET_DEFS: &[(u16, ClickProc, u16, u16, i16, i16, i16, u16, u16, u16)] = &[
    // index, click, shortcut, flags, spriteID, offsetX, offsetY, width, height, stringID
    (1, ClickProc::Mentat, 112, 1237, 7, 16, 1, 78, 16, STR_NULL),
    (2, ClickProc::Options, 113, 1237, 9, 104, 1, 78, 16, STR_NULL),
    (3, ClickProc::Name, 0, 17605, -1, 256, 42, 64, 6, STR_NULL),
    (6, ClickProc::Picture, 114, 17605, -4, 258, 51, 32, 24, STR_NULL),
    (4, ClickProc::RepairUpgrade, 0, 17605, -2, 258, 76, 60, 10, STR_NULL),
    (5, ClickProc::SpriteTextButton, 0, 17605, -3, 258, 87, 60, 35, STR_NULL),
    (7, ClickProc::Cancel, 110, 17605, -2, 258, 110, 58, 11, STR_CANCEL),
    (8, ClickProc::TextButton, 0, 17605, -2, 258, 77, 60, 10, STR_NULL),
    (9, ClickProc::TextButton, 0, 17605, -2, 258, 88, 60, 10, STR_NULL),
    (10, ClickProc::TextButton, 0, 17605, -2, 258, 99, 60, 10, STR_NULL),
    (11, ClickProc::TextButton, 0, 17605, -2, 258, 110, 60, 10, STR_NULL),
    (39, ClickProc::Viewport, 0, 20224, -1, 0, 24, 240, 16, STR_NULL),
    (40, ClickProc::Viewport, 0, 20224, -1, 240, 40, 10, 160, STR_NULL),
    (41, ClickProc::Viewport, 0, 20224, -1, 0, 40, 2, 160, STR_NULL),
    (42, ClickProc::Viewport, 0, 20224, -1, 0, 198, 240, 2, STR_NULL),
    (43, ClickProc::Viewport, 0, 20224, -1, 0, 40, 240, 160, STR_NULL),
    (44, ClickProc::Viewport, 0, 20288, -1, 256, 136, 64, 64, STR_NULL),
    (45, ClickProc::Viewport, 0, 20224, -1, 0, 0, 320, 200, STR_NULL),
];

impl GameUi {
    pub fn new(assets: Assets, game: Game) -> GameUi {
        let fonts = Fonts::load(&assets.vfs);
        let palette = assets.palette.0;
        let mut pal_map1 = palette_create_mapping(&palette, 0xC, 0x55);
        pal_map1[0xFF] = 0xFF;
        pal_map1[0xDF] = 0xDF;
        pal_map1[0xEF] = 0xEF;
        let mut pal_map2 = palette_create_mapping(&palette, 0xF, 0x55);
        pal_map2[0xFF] = 0xFF;
        pal_map2[0xDF] = 0xDF;
        pal_map2[0xEF] = 0xEF;
        let mut ui = GameUi {
            game,
            assets,
            screen: Screen::new(),
            text_screen: Screen::new(),
            fonts,
            widgets: Widgets::default(),
            input: Input::default(),
            palette,
            remap: palette_create_remap(HOUSE_MERCENARY),
            pal_map1,
            pal_map2,
            interface: Vec::new(),
            cursor_sprite: 0,
            cursor_default: 0,
            cursor_hotspot: (0, 0),
            tick_cursor: 0,
            tick_map_scroll: 0,
            tick_click: 0,
            var_37b8: false,
            text: TextDisplay { importance: [-1; 3], colours: [12; 3], ..Default::default() },
            production_string_id: STR_NULL,
            structure_high_health: true,
            credits_animation: 0,
            credits_animation_offset: 0,
            tick_credits: 0,
            pal_timer_animation: 0,
            pal_timer_selection: 0,
            pal_timer_toggle: 0,
            pal_animation_toggle: false,
            pal_selection_colour: 15,
            pal_toggle_colour: 12,
            hints_shown: 0,
            modal: None,
            factory: None,
            sounds: Vec::new(),
            audio: None,
            music: Default::default(),
            qol: QolSettings::default(),
            options: None,
            auto_scroll: true,
            sim_percent: 100,
            debug_full_radar: false,
            sim_acc: 0,
            setup: None,
            window_art: None,
        };
        ui.create_widgets();
        ui
    }

    /// Window_WidgetClick_Create
    fn create_widgets(&mut self) {
        self.widgets = Widgets::default();
        for &(index, click, shortcut, flags, sprite_id, ox, oy, w, h, string_id) in WIDGET_DEFS {
            let kind = match sprite_id + 4 {
                0 => WidgetKind::SpriteButton,
                1 => WidgetKind::SpriteTextButton,
                2 => WidgetKind::TextButton2,
                3 => WidgetKind::None,
                _ => WidgetKind::Sprite { normal: sprite_id as u16, down: sprite_id as u16 + 1 },
            };
            let (mut width, mut height) = (w, h);
            if let WidgetKind::Sprite { normal, .. } = kind {
                if let Some(s) = self.sprite(normal) {
                    width = s.width;
                    height = s.height as u16;
                }
            }
            let mut sc = shortcut;
            if kind == WidgetKind::SpriteTextButton && string_id != STR_NULL {
                sc = get_shortcut(*self.game.strings.get(string_id).first().unwrap_or(&0));
            }
            let mut wd = Widget {
                index,
                shortcut: sc,
                shortcut2: sc,
                kind,
                click,
                string_id,
                requires_click: flags & 0x1 != 0,
                click_as_hover: flags & 0x4 != 0,
                invisible: flags & 0x8 != 0,
                grey_when_invisible: flags & 0x10 != 0,
                no_click_cascade: flags & 0x20 != 0,
                lose_select: flags & 0x40 != 0,
                button_filter_left: (flags >> 8) & 0xF,
                button_filter_right: (flags >> 12) & 0xF,
                parent_id: 0,
                offset_x: ox,
                offset_y: oy,
                width,
                height,
                selected: false,
                hover1: false,
                hover2: false,
                selected_last: false,
                hover1_last: false,
                key_selected: false,
                button_state: 0,
            };
            if string_id == STR_CANCEL {
                wd.shortcut2 = b'n' as u16;
            }
            self.widgets.insert(wd);
        }
    }

    pub fn sprite(&self, id: u16) -> Option<&Sprite> {
        self.assets.sprites.get(id as usize).and_then(|s| s.as_ref())
    }

    /// Start a new game: set up house colours, the interface and the selection.
    pub fn start(&mut self) {
        self.remap = palette_create_remap(self.game.player_house_id);
        self.build_interface();
        self.text = TextDisplay { importance: [-1; 3], colours: [12; 3], ..Default::default() };
        let p = self.game.player_house_id;
        self.credits_animation = self.game.h(p).credits;
        self.game.player_credits = self.credits_animation;
        self.game.selection_type = SelectionType::Mentat;
        self.change_selection_type(SelectionType::Structure);
        self.factory = None;
        self.modal = None;
    }

    /// GUI_DrawInterfaceAndRadar: the static interface background.
    fn build_interface(&mut self) {
        let mut s = Screen::new();
        if let Some(cps) = self.assets.vfs.get("SCREEN.CPS") {
            s.pixels = crate::data::image::load_cps(cps).pixels;
        }
        let credits = self.sprite(11).cloned();
        s.draw_sprite(credits.as_ref(), 192, 0, 0, 0, None, None);
        s.remap_area(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, &self.remap);
        self.interface = s.pixels;
    }

    /// GUI_ChangeSelectionType (UI side) plus the game state side.
    pub fn change_selection_type(&mut self, t: SelectionType) {
        let old = self.game.selection_type;
        self.game.change_selection_type(t);
        let new = self.game.selection_type;
        if old == new {
            return;
        }
        self.var_37b8 = true;
        if matches!(old, SelectionType::Place | SelectionType::Target | SelectionType::Structure) {
            self.cursor_default = 0;
            self.display_text_clear();
        }
        let visible: &[u16] = &[45, 2, 1, 44, 43, 42, 41, 40, 39];
        for w in self.widgets.list.iter_mut() {
            w.selected = false;
            w.invisible = !visible.contains(&w.index);
        }
        self.text.needs_update = true;
        if new == SelectionType::Target {
            self.cursor_default = 5;
        }
    }

    // ---------------------------------------------------------------------------------------
    // Per-tick update
    // ---------------------------------------------------------------------------------------

    /// One 60 Hz tick of the main loop (GameLoop_Main with the game running).
    pub fn tick(&mut self) {
        if self.setup.is_some() {
            self.setup_tick();
            return;
        }
        self.tick_inner();
        self.audio_tick();
    }

    fn tick_inner(&mut self) {
        self.palette_animate();
        if let Some(m) = &mut self.modal {
            // GUI_DisplayModalMessage: wait 30 ticks, then any click/key closes it.
            self.game.timer_gui = self.game.timer_gui.wrapping_add(1);
            if m.wait > 0 {
                m.wait -= 1;
                self.input.queue.clear();
            } else {
                while let Some(k) = self.input.queue.pop_front() {
                    if k != 0 && k & 0x800 == 0 {
                        self.modal = None;
                        break;
                    }
                }
            }
            return;
        }
        if self.factory.is_some() {
            self.game.timer_gui = self.game.timer_gui.wrapping_add(1);
            self.factory_tick();
            return;
        }
        if self.options.is_some() {
            self.options_tick();
            return;
        }

        // GUI_Widget_HandleEvents with the click procs.
        let mut widgets = std::mem::take(&mut self.widgets);
        let mut input = std::mem::take(&mut self.input);
        // QoL: hovering the far right edge acts as hovering the right scroll strip (widget 40).
        let real_mouse_x = input.mouse_x;
        let edge = self.qol.screen_edge_scroll
            && input.mouse_x >= SCREEN_WIDTH as i32 - 2
            && (40..SCREEN_HEIGHT as i32).contains(&input.mouse_y)
            && !input.left_down
            && !input.right_down;
        if edge {
            input.mouse_x = 245;
        }
        self.input = input.snapshot();
        let key = widgets.handle_events(&mut input, &mut |ws, i| self.click(ws, i));
        self.widgets = widgets;
        if edge && input.mouse_x == 245 {
            input.mouse_x = real_mouse_x;
        }
        self.input = input;

        if matches!(self.game.selection_type, SelectionType::Target | SelectionType::Place | SelectionType::Unit | SelectionType::Structure) {
            // GUI_Widget_ActionPanel_Draw: also decides which panel buttons exist.
            self.draw_action_panel();
            self.numpad_move(key);
        }

        self.sim_acc += self.sim_percent.max(10);
        while self.sim_acc >= 100 {
            self.sim_acc -= 100;
            self.game.tick();
            if self.modal.is_some() || self.factory.is_some() {
                self.sim_acc = 0;
                break;
            }
        }
        self.process_events();
        self.credits_tick();
        self.display_text_tick();
    }

    pub(crate) fn process_events(&mut self) {
        let events = std::mem::take(&mut self.game.events);
        for e in events {
            match e {
                Event::Text(t, p) => self.display_text(&t, p),
                Event::Feedback(id) => self.sounds.push(SoundRequest::Feedback(id)),
                Event::VoiceAtTile(id, pos) => {
                    let volume = self.voice_volume(pos);
                    self.sounds.push(SoundRequest::VoiceAtTile(id, volume));
                }
                Event::Sound(id) => self.sounds.push(SoundRequest::StartSound(id)),
                Event::Hint(string_id, sprite) => self.display_hint(string_id, sprite),
                Event::FactoryWindow { structure, construction_yard, starport, upgrade_cost } => {
                    self.factory = Some(factory::FactoryWindow::new(self, structure, construction_yard, starport, upgrade_cost));
                }
                Event::SelectionType(_) => {}
                Event::Radar(_) | Event::ScreenShake | Event::Music(_) => {}
            }
        }
    }

    /// GUI_DisplayHint
    fn display_hint(&mut self, string_id: u16, sprite: u16) {
        if self.qol.no_hints || string_id == STR_NULL || self.game.selection_type == SelectionType::Mentat {
            return;
        }
        let hint = string_id.wrapping_sub(STR_YOU_MUST_BUILD_A_WINDTRAP_TO_PROVIDE_POWER_TO_YOUR_BASE_WITHOUT_POWER_YOUR_STRUCTURES_WILL_DECAY);
        if hint >= 64 {
            return;
        }
        if self.hints_shown & (1 << hint) != 0 {
            return;
        }
        self.hints_shown |= 1 << hint;
        let text = self.game.string(string_id);
        self.modal = Some(Modal { text, sprite, wait: 30 });
    }

    /// InGame_Numpad_Move
    fn numpad_move(&mut self, key: u16) {
        let dir = match key & 0xFF {
            0x10 => {
                self.map_select_next(key & 0x100 == 0);
                return;
            }
            0x5C | 0x4F => 6,
            0x66 | 0x59 => 2,
            0x60 | 0x53 => 0,
            0x62 | 0x54 => 4,
            0x5B | 0x50 => 7,
            0x5D | 0x51 => 5,
            0x65 | 0x55 => 1,
            0x67 | 0x56 => 3,
            _ => return,
        };
        self.map_move_direction(dir);
    }

    /// Map_MoveDirection
    pub fn map_move_direction(&mut self, direction: u16) {
        const OFF: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
        let g = &mut self.game;
        let (min_x, min_y, sx, sy) = g.map_info();
        let mut x = packed_x(g.minimap_position) as i32 + OFF[direction as usize].0;
        let mut y = packed_y(g.minimap_position) as i32 + OFF[direction as usize].1;
        x = x.max(min_x as i32).min((min_x + sx) as i32 - 15);
        y = y.max(min_y as i32).min((min_y + sy) as i32 - 10);
        g.viewport_position = pack_xy(x as u16, y as u16);
        g.minimap_position = g.viewport_position;
    }

    /// Map_SelectNext (TAB)
    fn map_select_next(&mut self, get_next: bool) {
        let g = &self.game;
        let visible = |p: u16| {
            let (x, y) = (packed_x(p), packed_y(p));
            let (x2, y2) = (packed_x(g.minimap_position), packed_y(g.minimap_position));
            x >= x2 && x < x2 + 15 && y >= y2 && y < y2 + 10
        };
        let ph = g.player_house_id;
        let mut objs: Vec<(Tile32, bool)> = Vec::new();
        for u in g.units_of(HOUSE_INVALID, 0xFFFF) {
            let un = g.u(u);
            if !g.ui(u).o.flags.tab_selectable || !visible(un.o.position.pack()) || un.o.seen_by_houses & (1 << ph) == 0 {
                continue;
            }
            objs.push((un.o.position, g.unit_selected == Some(u)));
        }
        for s in g.structures_of(HOUSE_INVALID, 0xFFFF) {
            let st = g.s(s);
            if matches!(st.o.type_, STRUCTURE_SLAB_1X1 | STRUCTURE_SLAB_2X2 | STRUCTURE_WALL) || !visible(st.o.position.pack()) || st.o.seen_by_houses & (1 << ph) == 0 {
                continue;
            }
            objs.push((st.o.position, g.unit_selected.is_none() && g.structure_get_by_packed_tile(g.selection_position) == Some(s)));
        }
        if objs.is_empty() {
            return;
        }
        let cur = objs.iter().position(|o| o.1).unwrap_or(0);
        let n = objs.len();
        let next = if get_next { (cur + 1) % n } else { (cur + n - 1) % n };
        let p = objs[next].0.pack();
        self.game.map_set_selection(p);
    }

    // ---------------------------------------------------------------------------------------
    // Click procs
    // ---------------------------------------------------------------------------------------

    fn click(&mut self, ws: &mut Widgets, i: usize) -> bool {
        let w = ws.list[i].clone();
        if std::env::var("UIDBG").is_ok() && w.button_state & 0x33 != 0 {
            eprintln!("click widget {} bs={:x} mouse=({},{}) click=({},{})", w.index, w.button_state, self.input.mouse_x, self.input.mouse_y, self.input.click_x, self.input.click_y);
        }
        match w.click {
            ClickProc::Viewport => self.click_viewport(&w),
            ClickProc::TextButton => self.click_text_button(ws, &w),
            ClickProc::SpriteTextButton => self.click_sprite_text_button(),
            ClickProc::Picture => {
                if let Some(u) = self.game.unit_selected {
                    self.game.unit_display_status_text(u);
                    self.process_events();
                    return false;
                }
                if let Some(s) = self.game.structure_get_by_packed_tile(self.game.selection_position) {
                    if self.game.si(s).o.flags.factory {
                        self.game.structure_build_object(s, 0xFFFF);
                        self.process_events();
                    }
                }
                false
            }
            ClickProc::RepairUpgrade => {
                if let Some(s) = self.game.structure_get_by_packed_tile(self.game.selection_position) {
                    if !self.game.structure_set_repairing_state(s, -1) {
                        self.game.structure_set_upgrading_state(s, -1);
                    }
                }
                false
            }
            ClickProc::Cancel => self.click_cancel(),
            ClickProc::Name => {
                if let Some(o) = self.game.object_get_by_packed_tile(self.game.selection_position) {
                    let p = self.game.obj(o).position.pack();
                    self.game.map_set_viewport_position(p);
                    self.game.minimap_position = self.game.viewport_position;
                    self.game.map_set_selection(p);
                }
                false
            }
            ClickProc::Mentat => false,
            ClickProc::Options => {
                self.options_open();
                false
            }
            ClickProc::None => false,
        }
    }

    /// GUI_Widget_Viewport_Click
    fn click_viewport(&mut self, w: &Widget) -> bool {
        const HOTSPOTS: [(i32, i32); 6] = [(0, 0), (5, 0), (8, 5), (5, 8), (0, 5), (8, 8)];
        let sprite_id = match w.index {
            39 => 1,
            40 => 2,
            41 => 4,
            42 => 3,
            43 | 44 => self.cursor_default,
            45 => 0,
            _ => self.cursor_sprite,
        };
        if sprite_id != self.cursor_sprite {
            self.tick_cursor = self.game.timer_game;
            self.cursor_hotspot = HOTSPOTS[sprite_id as usize];
            self.cursor_sprite = sprite_id;
        }
        if w.index == 45 {
            return true;
        }
        let mut click = false;
        let mut drag = false;
        if w.button_state & 0x11 != 0 {
            click = true;
            self.var_37b8 = false;
        } else if w.button_state & 0x22 != 0 && !self.var_37b8 {
            drag = true;
        }
        let right = w.button_state & 0x10 != 0 || w.button_state & 0x20 != 0;
        if self.game.config.dune2_enhanced && (click || drag) {
            if self.tick_click + 2 >= self.game.timer_game {
                return true;
            }
            self.tick_click = self.game.timer_game;
        }
        let direction = match w.index {
            39 => 0,
            40 => 2,
            41 => 6,
            42 => 4,
            _ => 0xFFFF,
        };
        if direction != 0xFFFF {
            if !click && !drag {
                if self.tick_map_scroll + 10 >= self.game.timer_game || self.tick_cursor + 20 >= self.game.timer_game {
                    return true;
                }
                if !self.auto_scroll && matches!(self.game.selection_type, SelectionType::Structure | SelectionType::Unit) {
                    return true;
                }
            }
            self.tick_map_scroll = self.game.timer_game;
            self.map_move_direction(direction);
            return true;
        }
        let (mut x, mut y) = if click { (self.input.click_x, self.input.click_y) } else { (self.input.mouse_x, self.input.mouse_y) };
        if w.index == 43 {
            x = x / 16 + packed_x(self.game.minimap_position) as i32;
            y = (y - 40) / 16 + packed_y(self.game.minimap_position) as i32;
        } else if w.index == 44 {
            let scale = self.game.scenario.map_scale as i32;
            let (min_x, min_y, sx, sy) = self.game.map_info();
            x = ((x.max(256) - 256) / (scale + 1)).min(sx as i32 - 1) + min_x as i32;
            y = ((y.max(136) - 136) / (scale + 1)).min(sy as i32 - 1) + min_y as i32;
        }
        let packed = pack_xy(x.clamp(0, 63) as u16, y.clamp(0, 63) as u16);

        if click && right && (self.qol.right_click_cancel || self.qol.right_click_orders) && w.index == 43 {
            if matches!(self.game.selection_type, SelectionType::Target | SelectionType::Place) {
                return self.click_cancel();
            }
            if self.qol.right_click_orders && self.right_click_order(packed) {
                return true;
            }
            if self.game.unit_selected.is_some() {
                self.game.unit_select(None);
            }
            return true;
        }

        if click && self.game.selection_type == SelectionType::Target {
            self.display_text_clear();
            if self.game.unit_house_missile.is_some() {
                self.game.unit_launch_house_missile(packed);
                self.process_events();
                return true;
            }
            let Some(u) = self.game.unit_active else { return true };
            let action = self.game.active_action as u8;
            self.issue_order(u, action, packed);
            self.game.unit_active = None;
            self.game.active_action = 0xFFFF;
            self.change_selection_type(SelectionType::Unit);
            return true;
        }

        if click && self.game.selection_type == SelectionType::Place {
            let Some(s) = self.game.structure_active else { return true };
            let si = self.game.structure_info[self.game.structure_active_type as usize];
            let ph = self.game.player_house_id;
            let pos = self.game.selection_position;
            if self.game.structure_place(s, pos) {
                self.sounds.push(SoundRequest::VoiceAtTile(20, 255));
                if self.game.s(s).o.type_ == STRUCTURE_PALACE {
                    let p = self.game.s(s).o.position;
                    self.game.hm(ph).palace_position = p;
                }
                if self.game.structure_active_type == STRUCTURE_REFINERY as u16 && self.game.validate_strict_if_zero == 0 {
                    self.game.validate_strict_if_zero += 1;
                    let e = self.game.structure_encode(s);
                    let u = self.game.unit_create_wrapper(ph, UNIT_HARVESTER, e);
                    self.game.validate_strict_if_zero -= 1;
                    match u {
                        None => self.game.hm(ph).harvesters_incoming += 1,
                        Some(u) => self.game.um(u).origin_encoded = e,
                    }
                }
                self.change_selection_type(SelectionType::Structure);
                if let Some(cy) = self.game.structure_get_by_packed_tile(self.game.structure_active_position) {
                    let ot = self.game.s(cy).object_type;
                    if self.game.structure_get_buildable(cy) & (1u32.wrapping_shl(ot as u32)) == 0 {
                        self.game.structure_build_object(cy, 0xFFFE);
                    }
                }
                self.game.structure_active_type = 0xFFFF;
                self.game.structure_active = None;
                self.game.selection_state = 0;
                self.display_hint(si.o.hint_string_id, si.o.sprite_id);
                self.game.house_update_radar_state(ph);
                let h = self.game.h(ph);
                if h.power_production < h.power_usage && h.structures_built & (1 << STRUCTURE_OUTPOST) != 0 {
                    let t = self.game.string(STR_NOT_ENOUGH_POWER_FOR_RADAR_BUILD_WINDTRAPS);
                    self.display_text(&t, 3);
                }
                self.process_events();
                return true;
            }
            self.sounds.push(SoundRequest::VoiceAtTile(47, 255));
            let t = self.game.structure_active_type as u8;
            if t == STRUCTURE_SLAB_1X1 || t == STRUCTURE_SLAB_2X2 {
                let s = self.game.string(STR_CAN_NOT_PLACE_FOUNDATION_HERE);
                self.display_text(&s, 2);
            } else {
                self.display_hint(STR_STRUCTURES_MUST_BE_PLACED_ON_CLEAR_ROCK_OR_CONCRETE_AND_ADJACENT_TO_ANOTHER_FRIENDLY_STRUCTURE, 0xFFFF);
                let fmt = self.game.string(STR_CAN_NOT_PLACE_S_HERE);
                let name = self.game.string(si.o.string_id_abbrev);
                let s = format_c(&fmt, &[Arg::S(&name)]);
                self.display_text(&s, 2);
            }
            return true;
        }

        if click && w.index == 43 {
            let position = self.game.unit_find_target_around(packed);
            if self.game.map[position as usize].overlay_tile_id != self.game.veiled_tile_id && self.game.object_get_by_packed_tile(position).is_some() {
                self.game.map_set_selection(position);
                if let Some(u) = self.game.unit_selected {
                    self.game.unit_display_status_text(u);
                }
                self.process_events();
            }
            if w.button_state & 0x10 != 0 {
                self.game.map_set_viewport_position(packed);
                self.game.minimap_position = self.game.viewport_position;
            }
            return true;
        }
        if (click || drag) && w.index == 44 {
            self.game.map_set_viewport_position(packed);
            self.game.minimap_position = self.game.viewport_position;
            return true;
        }
        if self.game.selection_type == SelectionType::Target {
            let t = self.game.unit_find_target_around(packed);
            self.game.map_set_selection(t);
        } else if self.game.selection_type == SelectionType::Place {
            self.game.map_set_selection(packed);
        }
        true
    }

    /// QoL: pick an order for the selected unit from what was right-clicked, and issue it the
    /// same way the original does after "Attack"/"Move" + a click on the map.
    fn right_click_order(&mut self, packed: u16) -> bool {
        let g = &self.game;
        let Some(u) = g.unit_selected else { return false };
        if g.unit_get_house_id(u) != g.player_house_id || g.u(u).deviated != 0 {
            return false;
        }
        let actions = g.ui(u).o.actions_player;
        let can = |a: u8| actions.iter().any(|&x| x as u8 == a);
        let target = g.unit_find_target_around(packed);
        let visible = g.map[target as usize].overlay_tile_id != g.veiled_tile_id;
        let enemy = visible
            && match g.object_get_by_packed_tile(target) {
                Some(ObjRef::Unit(t)) => t != u && !g.house_are_allied(g.unit_get_house_id(t), g.player_house_id),
                Some(ObjRef::Structure(t)) => !g.house_are_allied(g.s(t).o.house_id, g.player_house_id),
                None => false,
            };
        let lst = g.map_get_landscape_type(packed);
        let action = if enemy && can(ACTION_ATTACK) {
            ACTION_ATTACK
        } else if can(ACTION_HARVEST) && (lst == LST_SPICE || lst == LST_THICK_SPICE) {
            ACTION_HARVEST
        } else if can(ACTION_MOVE) {
            ACTION_MOVE
        } else {
            return false;
        };
        self.issue_order(u, action, packed);
        true
    }

    /// The order part of GUI_Widget_Viewport_Click in SELECTIONTYPE_TARGET.
    fn issue_order(&mut self, u: u16, action: u8, packed: u16) {
        self.game.object_script_variable4_clear(ObjRef::Unit(u));
        {
            let unit = self.game.um(u);
            unit.target_attack = 0;
            unit.target_move = 0;
            unit.route[0] = 0xFF;
        }
        let encoded = if action != ACTION_MOVE && action != ACTION_HARVEST {
            let t = self.game.unit_find_target_around(packed);
            self.game.tile_encode(t)
        } else {
            self.game.tile_encode(packed)
        };
        self.game.unit_set_action(u, action);
        if action == ACTION_MOVE {
            self.game.unit_set_destination(u, encoded);
        } else if action == ACTION_HARVEST {
            self.game.um(u).target_move = encoded;
        } else {
            self.game.unit_set_target(u, encoded);
            if let Some(t) = self.game.index_get_unit(self.game.u(u).target_attack) {
                self.game.um(t).blink_counter = 8;
            }
        }
        if self.game.ui(u).movement_type == MOVEMENT_FOOT {
            self.sounds.push(SoundRequest::OrderAck(ACTION_INFO[action as usize].sound_id));
        } else {
            let v = if self.game.rng.random_256() & 1 == 0 { 20 } else { 17 };
            self.sounds.push(SoundRequest::OrderAck(v));
        }
    }

    /// GUI_Widget_TextButton_Click (unit commands)
    fn click_text_button(&mut self, ws: &mut Widgets, w: &Widget) -> bool {
        let Some(u) = self.game.unit_selected else { return true };
        let ui = *self.game.ui(u);
        let actions = if self.game.unit_get_house_id(u) != self.game.player_house_id && self.game.u(u).o.type_ != UNIT_HARVESTER { ACTIONS_AI } else { ui.o.actions_player };
        let mut action = actions[(w.index - 8) as usize] as u8;
        if self.game.config.dune2_enhanced && (self.input.test(0x2c) || self.input.test(0x39)) {
            if action == ACTION_GUARD {
                action = ACTION_AREA_GUARD;
            } else if action == ACTION_ATTACK {
                action = ACTION_AMBUSH;
            }
        }
        let mut unit_action = self.game.u(u).next_action_id;
        if unit_action == ACTION_INVALID {
            unit_action = self.game.u(u).action_id;
        }
        if self.game.u(u).deviated != 0 {
            self.game.unit_deviation_decrease(u, 5);
            if self.game.u(u).deviated == 0 {
                ws.make_selected(w.index, false);
                return true;
            }
        }
        ws.make_selected(w.index, true);
        let ai = ACTION_INFO[action as usize];
        let st = match ai.selection_type {
            0 => SelectionType::Mentat,
            1 => SelectionType::Target,
            2 => SelectionType::Place,
            3 => SelectionType::Unit,
            _ => SelectionType::Structure,
        };
        if st != self.game.selection_type {
            self.game.unit_active = self.game.unit_selected;
            self.game.active_action = action as u16;
            self.change_selection_type(st);
            return true;
        }
        self.game.object_script_variable4_clear(ObjRef::Unit(u));
        {
            let unit = self.game.um(u);
            unit.target_attack = 0;
            unit.target_move = 0;
            unit.route[0] = 0xFF;
        }
        self.game.unit_set_action(u, action);
        if ui.movement_type == MOVEMENT_FOOT {
            self.sounds.push(SoundRequest::StartSound(ai.sound_id));
        }
        if unit_action == action {
            return true;
        }
        if let Some(p) = actions.iter().position(|&a| a as u8 == unit_action) {
            ws.make_selected(p as u16 + 8, false);
        }
        true
    }

    /// GUI_Widget_SpriteTextButton_Click (Build It / Place It / On Hold / specials)
    fn click_sprite_text_button(&mut self) -> bool {
        let Some(s) = self.game.structure_get_by_packed_tile(self.game.selection_position) else { return false };
        match self.production_string_id {
            STR_PLACE_IT => {
                if self.game.s(s).o.type_ == STRUCTURE_CONSTRUCTION_YARD {
                    let ns = self.game.s(s).o.linked_id as u16;
                    self.game.structure_active = Some(ns);
                    self.game.structure_active_type = self.game.s(s).object_type;
                    let t = self.game.structure_active_type as u8;
                    self.game.selection_state = self.game.structure_is_valid_build_location(self.game.selection_position, t);
                    self.game.structure_active_position = self.game.selection_position;
                    self.game.sm(s).o.linked_id = 0xFF;
                    self.change_selection_type(SelectionType::Place);
                }
            }
            STR_ON_HOLD => {
                let f = &mut self.game.sm(s).o.flags;
                f.repairing = false;
                f.on_hold = false;
                f.upgrading = false;
            }
            STR_BUILD_IT => {
                let t = self.game.s(s).object_type;
                self.game.structure_build_object(s, t);
            }
            STR_LAUNCH | STR_FREMEN | STR_SABOTEUR => self.game.structure_activate_special(s),
            STR_D_DONE => self.game.sm(s).o.flags.on_hold = true,
            _ => {}
        }
        self.process_events();
        false
    }

    /// GUI_Widget_Cancel_Click
    fn click_cancel(&mut self) -> bool {
        if self.game.structure_active_type != 0xFFFF {
            let s = self.game.structure_get_by_packed_tile(self.game.structure_active_position);
            if let Some(s2) = self.game.structure_active {
                match s {
                    Some(s) => self.game.sm(s).o.linked_id = (s2 & 0xFF) as u8,
                    None => self.game.structure_free(s2),
                }
            }
            self.game.structure_active = None;
            self.game.structure_active_type = 0xFFFF;
            self.change_selection_type(SelectionType::Structure);
            self.game.selection_state = 0;
        }
        if self.game.unit_active.is_none() {
            return true;
        }
        self.game.unit_active = None;
        self.game.active_action = 0xFFFF;
        self.cursor_sprite = 0;
        self.cursor_hotspot = (0, 0);
        self.change_selection_type(SelectionType::Unit);
        true
    }

    // ---------------------------------------------------------------------------------------
    // Text line (GUI_DisplayText)
    // ---------------------------------------------------------------------------------------

    pub fn display_text_clear(&mut self) {
        self.text.importance = [-1; 3];
        self.text.lines = Default::default();
        self.text.scroll_in_progress = false;
        self.text.display_timer = 0;
    }

    pub fn display_text(&mut self, s: &[u8], importance: i16) {
        let mut buffer: Vec<u8> = s.iter().copied().take(79).collect();
        if importance == -1 {
            self.display_text_clear();
            return;
        }
        let t = &mut self.text;
        if importance == -2 {
            if !t.scroll_in_progress {
                t.importance[1] = -1;
                t.lines[1].clear();
            }
            t.importance[2] = -1;
            t.lines[2].clear();
        }
        let eq = |a: &[u8], b: &[u8]| a.eq_ignore_ascii_case(b);
        if t.scroll_in_progress {
            if !buffer.is_empty() && !eq(&buffer, &t.lines[1]) && importance >= t.importance[2] {
                t.lines[2] = std::mem::take(&mut buffer);
                t.importance[2] = importance;
            }
            return;
        }
        if !buffer.is_empty() {
            if !eq(&buffer, &t.lines[0]) && !eq(&buffer, &t.lines[1]) && !eq(&buffer, &t.lines[2]) {
                if importance >= t.importance[1] {
                    t.lines[2] = std::mem::take(&mut t.lines[1]);
                    t.colours[2] = t.colours[1];
                    t.importance[2] = t.importance[1];
                    t.lines[1] = buffer;
                    t.colours[1] = 12;
                    t.importance[1] = importance;
                } else if importance >= t.importance[2] {
                    t.lines[2] = buffer;
                    t.importance[2] = importance;
                    t.colours[2] = 12;
                }
            }
        } else if t.lines[0].is_empty() && t.lines[1].is_empty() {
            return;
        }
        if t.importance[1] <= t.importance[0] && t.display_timer >= self.game.timer_gui {
            return;
        }
        t.scroll_in_progress = true;
        t.text_offset = 10;
        t.display_timer = 0;
    }

    /// GUI_DisplayText(NULL, 0) each loop: advance the scrolling.
    fn display_text_tick(&mut self) {
        let now = self.game.timer_gui;
        let t = &mut self.text;
        if t.scroll_in_progress {
            if t.display_timer > now {
                return;
            }
            if t.text_offset != 0 {
                if t.importance[2] <= t.importance[1] {
                    t.display_timer = now + 1;
                }
                t.text_offset -= 1;
                return;
            }
            t.lines[0] = std::mem::take(&mut t.lines[1]);
            t.colours[0] = t.colours[1];
            t.importance[0] = if t.importance[1] != 0 { t.importance[1] - 1 } else { 0 };
            t.lines[1] = std::mem::take(&mut t.lines[2]);
            t.importance[1] = t.importance[2];
            t.colours[1] = t.colours[2];
            t.importance[2] = -1;
            t.needs_update = true;
            t.display_timer = now + if t.importance[1] <= t.importance[0] { 900 } else { 1 };
            t.scroll_in_progress = false;
            return;
        }
        if t.lines[0].is_empty() && t.lines[1].is_empty() {
            return;
        }
        if t.importance[1] <= t.importance[0] && t.display_timer >= now {
            return;
        }
        t.scroll_in_progress = true;
        t.text_offset = 10;
        t.display_timer = 0;
    }

    fn draw_text_line(&mut self) {
        // The two lines are drawn on a 320x24 strip; rows [offset, offset+14) are shown at y=21.
        let fg_normal = WIDGET_PROPERTIES[7].5;
        let ts = &mut self.text_screen;
        ts.fill_rect(0, 0, SCREEN_WIDTH as i32 - 1, 23, fg_normal);
        let x = (WIDGET_PROPERTIES[7].0 * 8) as i32;
        let (l1, l2) = (self.text.lines[0].clone(), self.text.lines[1].clone());
        ts.draw_text_wrapper(&self.fonts, &l2, x, 2, self.text.colours[1], 0, 0x012);
        ts.draw_text_wrapper(&self.fonts, &l1, x, 13, self.text.colours[0], 0, 0x012);
        let offset = if self.text.scroll_in_progress { self.text.text_offset as usize } else { 11 };
        let (wx, wy, ww, wh) = (WIDGET_PROPERTIES[7].0 as usize * 8, WIDGET_PROPERTIES[7].1 as usize, WIDGET_PROPERTIES[7].2 as usize * 8, WIDGET_PROPERTIES[7].3 as usize);
        let h = if offset + wh > 24 { 24 - offset } else { wh };
        for row in 0..h {
            let src = (offset + row) * SCREEN_WIDTH + wx;
            let dst = (wy + row) * SCREEN_WIDTH + wx;
            self.screen.pixels[dst..dst + ww].copy_from_slice(&ts.pixels[src..src + ww]);
        }
    }

    // ---------------------------------------------------------------------------------------
    // Palette animation (GUI_PaletteAnimate)
    // ---------------------------------------------------------------------------------------

    fn shift_colour(palette: &mut [u8; 768], colour: usize, reference: usize) -> bool {
        let mut ret = false;
        for i in 0..3 {
            let (c, r) = (colour * 3 + i, reference * 3 + i);
            if palette[r] != palette[c] {
                ret = true;
                if palette[c] > palette[r] {
                    palette[c] -= 1;
                } else {
                    palette[c] += 1;
                }
            }
        }
        ret
    }

    fn palette_animate(&mut self) {
        let now = self.game.timer_gui;
        if self.pal_timer_animation < now {
            let colour = if !self.structure_high_health && self.pal_animation_toggle { 6 } else { 15 };
            for i in 0..3 {
                self.palette[239 * 3 + i] = self.palette[colour * 3 + i];
            }
            self.pal_animation_toggle = !self.pal_animation_toggle;
            self.pal_timer_animation = now + 60;
        }
        if self.pal_timer_selection < now && self.game.selection_type != SelectionType::Mentat {
            let c = self.pal_selection_colour as usize;
            Self::shift_colour(&mut self.palette, 255, c);
            Self::shift_colour(&mut self.palette, 255, c);
            Self::shift_colour(&mut self.palette, 255, c);
            if !Self::shift_colour(&mut self.palette, 255, c) {
                if self.pal_selection_colour == 13 {
                    self.pal_selection_colour = 15;
                    if self.game.selection_type == SelectionType::Place {
                        self.pal_selection_colour = if self.game.selection_state != 0 {
                            if self.game.selection_state < 0 {
                                5
                            } else {
                                15
                            }
                        } else {
                            6
                        };
                    }
                } else {
                    self.pal_selection_colour = 13;
                }
            }
            self.pal_timer_selection = now + 3;
        }
        if self.pal_timer_toggle < now {
            let c = self.pal_toggle_colour as usize;
            Self::shift_colour(&mut self.palette, 223, c);
            if !Self::shift_colour(&mut self.palette, 223, c) {
                self.pal_toggle_colour = if self.pal_toggle_colour == 12 { 10 } else { 12 };
            }
            self.pal_timer_toggle = now + 5;
        }
    }

    // ---------------------------------------------------------------------------------------
    // Credits counter (GUI_DrawCredits)
    // ---------------------------------------------------------------------------------------

    pub(crate) fn credits_tick(&mut self) {
        if self.tick_credits > self.game.timer_gui {
            return;
        }
        self.tick_credits = self.game.timer_gui + 1;
        let credits = self.game.h(self.game.player_house_id).credits;
        if credits == self.credits_animation && self.credits_animation_offset == 0 {
            return;
        }
        let diff = credits as i32 - self.credits_animation as i32;
        if diff != 0 {
            let mut d = (diff / 4) as i16;
            if d == 0 {
                d = if diff < 0 { -1 } else { 1 };
            }
            d = d.clamp(-128, 128);
            self.credits_animation_offset += d;
        } else {
            self.credits_animation_offset = 0;
        }
        if diff != 0 && (self.credits_animation_offset < -7 || self.credits_animation_offset > 7) {
            self.sounds.push(SoundRequest::Effect(if diff > 0 { 52 } else { 53 }));
        }
        if self.credits_animation_offset < 0 && self.credits_animation == 0 {
            self.credits_animation_offset = 0;
        }
        self.credits_animation = (self.credits_animation as i32 + (self.credits_animation_offset / 8) as i32) as u16;
        if self.credits_animation_offset > 0 {
            self.credits_animation_offset &= 7;
        }
        if self.credits_animation_offset < 0 {
            self.credits_animation_offset = -((-self.credits_animation_offset) & 7);
        }
    }

    pub(crate) fn draw_credits(&mut self) {
        let mut credits_old = self.credits_animation;
        let mut credits_new = self.credits_animation;
        let mut offset: i32 = 1;
        if self.credits_animation_offset < 0 {
            credits_old = credits_old.saturating_sub(1);
            offset -= 8;
        }
        if self.credits_animation_offset > 0 {
            credits_new += 1;
        }
        self.game.player_credits = credits_old;
        let bg = self.sprite(12).cloned();
        self.screen.draw_sprite(bg.as_ref(), 0, 0, 5, DRAWSPRITE_FLAG_WIDGETPOS, None, None);
        let old = format!("{credits_old:6}").into_bytes();
        let new = format!("{credits_new:6}").into_bytes();
        let digit = |c: u8| if c == b' ' { 13 } else { c as u16 - 34 };
        let ao = self.credits_animation_offset as i32;
        for i in 0..6 {
            let left = i as i32 * 10 + 4;
            let s_old = self.sprite(digit(old[i])).cloned();
            if old[i] != new[i] {
                self.screen.draw_sprite(s_old.as_ref(), left, offset - ao, 5, DRAWSPRITE_FLAG_WIDGETPOS, None, None);
                if ao == 0 {
                    continue;
                }
                let s_new = self.sprite(digit(new[i])).cloned();
                self.screen.draw_sprite(s_new.as_ref(), left, offset + 8 - ao, 5, DRAWSPRITE_FLAG_WIDGETPOS, None, None);
            } else {
                self.screen.draw_sprite(s_old.as_ref(), left, 1, 5, DRAWSPRITE_FLAG_WIDGETPOS, None, None);
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Rendering
    // ---------------------------------------------------------------------------------------

    /// Render the current frame into `self.screen` (indexed) and return it.
    pub fn render(&mut self) -> &[u8] {
        if self.setup.is_some() {
            return self.setup_render();
        }
        if let Some(mut f) = self.factory.take() {
            f.render(self);
            self.factory = Some(f);
            self.draw_cursor();
            return &self.screen.pixels;
        }
        self.screen.pixels.copy_from_slice(&self.interface);
        self.draw_viewport();
        self.draw_minimap();
        self.draw_action_panel();
        self.draw_widgets();
        self.draw_credits();
        self.draw_text_line();
        if let Some(m) = self.modal.take() {
            self.draw_modal(&m);
            self.modal = Some(m);
        }
        if self.options.is_some() {
            // The original shows the plain arrow cursor in its windows.
            self.options_render();
            let s = self.sprite(0).cloned();
            self.screen.draw_sprite(s.as_ref(), self.input.mouse_x, self.input.mouse_y, 0, 0, None, None);
            return &self.screen.pixels;
        }
        self.draw_cursor();
        &self.screen.pixels
    }

    fn draw_cursor(&mut self) {
        let s = self.sprite(self.cursor_sprite).cloned();
        let (x, y) = (self.input.mouse_x - self.cursor_hotspot.0, self.input.mouse_y - self.cursor_hotspot.1);
        self.screen.draw_sprite(s.as_ref(), x, y, 0, 0, None, None);
    }

    fn draw_modal(&mut self, m: &Modal) {
        let fonts = &self.fonts;
        self.screen.draw_text_wrapper(fonts, b"", 0, 0, 0, 0, 0x22);
        let wp = WIDGET_PROPERTIES[1];
        let width_chars = wp.2 as i32 - if m.sprite == 0xFFFF { 2 } else { 7 };
        let max_width = width_chars * 8 - 6;
        // GUI_SplitText
        let mut lines: Vec<Vec<u8>> = Vec::new();
        let text = &m.text;
        let mut i = 0;
        while i < text.len() {
            let start = i;
            let mut width = 0;
            while width < max_width && i < text.len() && text[i] != b'\r' {
                width += self.screen.char_width(fonts, text[i]);
                i += 1;
            }
            if width >= max_width {
                while i > start && i < text.len() && text[i] != b' ' && text[i] != b'\r' {
                    i -= 1;
                }
            }
            lines.push(text[start..i].to_vec());
            if i < text.len() {
                i += 1;
            }
        }
        let fh = self.screen.font_height(fonts);
        let height = fh * (lines.len() as i32).max(3) + 18;
        let (x, y, w) = ((wp.0 * 8) as i32, wp.1 as i32, (wp.2 * 8) as i32);
        self.screen.border(x, y, w, height, 1, true);
        self.screen.border(x + 4, y + 4, w - 8, height - 8, 2, false);
        let mut tx = x + 8;
        if m.sprite != 0xFFFF {
            let s = self.sprite(m.sprite).cloned();
            self.screen.draw_sprite(s.as_ref(), x + 7, y + 8, 0, 0, None, None);
            tx = x + 5 * 8;
        }
        for (n, l) in lines.iter().enumerate() {
            self.screen.draw_text(fonts, l, tx, y + 8 + n as i32 * fh, wp.4, 0);
        }
    }

    fn house_palette(sprite: &Sprite, house_id: u8) -> Option<[u8; 16]> {
        let r = sprite.remap.as_ref()?;
        let mut p = *r;
        if house_id != 0 {
            for v in p.iter_mut() {
                if (0x90..=0x98).contains(v) {
                    *v += house_id << 4;
                }
            }
        }
        Some(p)
    }

    /// GUI_Widget_Viewport_Draw
    fn draw_viewport(&mut self) {
        const VALUES_32A4: [[u16; 2]; 8] = [[0, 0], [1, 0], [2, 0], [3, 0], [4, 0], [3, 1], [2, 1], [1, 1]];
        let g = &self.game;
        let enhanced = g.config.dune2_enhanced;
        let vp = g.viewport_position;
        self.screen.clip = Clip { left: 0, top: 40, right: 239, bottom: 199 };
        for y in 0..10u16 {
            let top = (y << 4) as i32 + 40;
            for x in 0..15u16 {
                let cur = vp.wrapping_add(pack_xy(x, y)) & 0xFFF;
                let t = &g.map[cur as usize];
                let left = (x << 4) as i32;
                if g.veiled_tile_id == t.overlay_tile_id {
                    self.screen.fill_rect(left, top, left + 15, top + 15, 12);
                    continue;
                }
                self.screen.draw_tile(&self.assets.tiles, t.ground_tile_id, left, top, t.house_id, enhanced);
                if t.overlay_tile_id != 0 {
                    self.screen.draw_tile(&self.assets.tiles, t.overlay_tile_id, left, top, t.house_id, enhanced);
                }
            }
        }

        // Sandworms
        for u in g.units_of(HOUSE_INVALID, UNIT_SANDWORM as u16) {
            let un = g.u(u);
            if !g.map[un.o.position.pack() as usize].is_unveiled {
                continue;
            }
            let sprite = self.sprite(g.ui(u).ground_sprite_id).cloned();
            for p in [un.o.position, un.target_last, un.target_pre_last] {
                let (ok, x, y) = g.map_is_position_in_viewport(p);
                if ok {
                    self.screen.draw_sprite(sprite.as_ref(), x as i32, y as i32, 2, DRAWSPRITE_FLAG_BLUR | DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER, None, None);
                }
            }
            if g.unit_selected == Some(u) {
                let (ok, x, y) = g.map_is_position_in_viewport(un.o.position);
                if ok {
                    let s6 = self.sprite(6).cloned();
                    self.screen.draw_sprite(s6.as_ref(), x as i32, y as i32, 2, DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER, None, None);
                }
            }
        }

        // Selection rectangle
        let sel = g.selection_position;
        if g.unit_selected.is_none() && (g.structure_get_by_packed_tile(sel).is_some() || g.selection_type == SelectionType::Place) {
            let x1 = (packed_x(sel) as i32 - packed_x(g.minimap_position) as i32) << 4;
            let y1 = ((packed_y(sel) as i32 - packed_y(g.minimap_position) as i32) << 4) + 0x28;
            let x2 = x1 + ((g.selection_width as i32) << 4) - 1;
            let y2 = y1 + ((g.selection_height as i32) << 4) - 1;
            self.screen.wired_rect(x1, y1, x2, y2, 0xFF);
            if g.selection_state == 0 && g.selection_type == SelectionType::Place {
                self.screen.line(x1, y1, x2, y2, 0xFF);
                self.screen.line(x2, y1, x1, y2, 0xFF);
            }
        }

        // Ground units
        for &u in &g.unit_find.clone() {
            let un = g.u(u);
            if un.o.index < 20 || un.o.index > 101 || un.o.flags.is_not_on_map {
                continue;
            }
            let packed = un.o.position.pack();
            if !g.map[packed as usize].is_unveiled {
                continue;
            }
            let ui = *g.ui(u);
            let (ok, mut x, mut y) = g.map_is_position_in_viewport(un.o.position);
            if !ok {
                continue;
            }
            x += TILE_DIFF[0][un.wobble_index as usize].x as i16;
            y += TILE_DIFF[0][un.wobble_index as usize].y as i16;
            let (x, y) = (x as i32, y as i32);
            let mut orientation = orientation_256_to_8(un.orientation[0].current as u8) as usize;
            let mut index;
            let mut sprite_flags: u16 = 0;
            if un.sprite_offset >= 0 || ui.destroyed_sprite_id == 0 {
                const VALUES_32C4: [[u16; 2]; 8] = [[0, 0], [1, 0], [1, 0], [1, 0], [2, 0], [1, 1], [1, 1], [1, 1]];
                index = ui.ground_sprite_id;
                match ui.display_mode {
                    DISPLAYMODE_UNIT | DISPLAYMODE_ROCKET => {
                        if ui.movement_type != MOVEMENT_SLITHER {
                            index += VALUES_32A4[orientation][0];
                            sprite_flags = VALUES_32A4[orientation][1];
                        }
                    }
                    DISPLAYMODE_INFANTRY_3_FRAMES => {
                        const V: [u16; 4] = [0, 1, 0, 2];
                        index += VALUES_32C4[orientation][0] * 3;
                        index += V[(un.sprite_offset & 3) as usize];
                        sprite_flags = VALUES_32C4[orientation][1];
                    }
                    DISPLAYMODE_INFANTRY_4_FRAMES => {
                        index += VALUES_32C4[orientation][0] * 4;
                        index += (un.sprite_offset & 3) as u16;
                        sprite_flags = VALUES_32C4[orientation][1];
                    }
                    _ => sprite_flags = 0,
                }
            } else {
                index = (ui.destroyed_sprite_id as i32 - un.sprite_offset as i32 - 1) as u16;
                sprite_flags = 0;
            }
            if un.o.type_ != UNIT_SANDWORM && un.o.flags.is_highlighted {
                sprite_flags |= DRAWSPRITE_FLAG_REMAP;
            }
            if ui.o.flags.blur_tile {
                sprite_flags |= DRAWSPRITE_FLAG_BLUR;
            }
            sprite_flags |= DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER;
            let house = if un.deviated != 0 { un.deviated_house } else { g.unit_get_house_id(u) };
            let sprite = self.sprite(index).cloned();
            match sprite.as_ref().and_then(|s| Self::house_palette(s, house)) {
                Some(pal) => self.screen.draw_sprite(sprite.as_ref(), x, y, 2, sprite_flags | DRAWSPRITE_FLAG_PAL, Some(&pal), Some((&self.pal_map2, 1))),
                None => self.screen.draw_sprite(sprite.as_ref(), x, y, 2, sprite_flags, None, Some((&self.pal_map2, 1))),
            }
            if un.o.type_ == UNIT_HARVESTER && un.action_id == ACTION_HARVEST && un.sprite_offset >= 0 {
                let t = g.map_get_landscape_type(packed);
                if t == LST_SPICE || t == LST_THICK_SPICE {
                    const V: [[i32; 2]; 8] = [[0, 7], [-7, 6], [-14, 1], [-9, -6], [0, -9], [9, -6], [14, 1], [7, 6]];
                    let id = (un.sprite_offset % 3) as u16 + 0xDF + VALUES_32A4[orientation][0] * 3;
                    let s = self.sprite(id).cloned();
                    self.screen.draw_sprite(s.as_ref(), x + V[orientation][0], y + V[orientation][1], 2, VALUES_32A4[orientation][1] | DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER, None, None);
                }
            }
            if un.sprite_offset >= 0 && ui.turret_sprite_id != 0xFFFF {
                let (mut ox, mut oy) = (0, 0);
                orientation = orientation_256_to_8(un.orientation[if ui.o.flags.has_turret { 1 } else { 0 }].current as u8) as usize;
                match ui.turret_sprite_id {
                    0x8D => oy = -2,
                    0x92 => oy = -3,
                    0x7E => {
                        const V: [[i32; 2]; 8] = [[0, -5], [0, -5], [2, -3], [2, -1], [-1, -3], [-2, -1], [-2, -3], [-1, -5]];
                        ox = V[orientation][0];
                        oy = V[orientation][1];
                    }
                    0x88 => {
                        const V: [[i32; 2]; 8] = [[0, -4], [-1, -3], [2, -4], [0, -3], [-1, -3], [0, -3], [-2, -4], [1, -3]];
                        ox = V[orientation][0];
                        oy = V[orientation][1];
                    }
                    _ => {}
                }
                let id = ui.turret_sprite_id + VALUES_32A4[orientation][0];
                let s = self.sprite(id).cloned();
                let f = VALUES_32A4[orientation][1] | DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER;
                match s.as_ref().and_then(|s| Self::house_palette(s, g.unit_get_house_id(u))) {
                    Some(pal) => self.screen.draw_sprite(s.as_ref(), x + ox, y + oy, 2, f | DRAWSPRITE_FLAG_PAL, Some(&pal), None),
                    None => self.screen.draw_sprite(s.as_ref(), x + ox, y + oy, 2, f, None, None),
                }
            }
            if un.o.flags.is_smoking {
                let mut id = 180 + (un.sprite_offset & 3) as u16;
                if id == 183 {
                    id = 181;
                }
                let s = self.sprite(id).cloned();
                self.screen.draw_sprite(s.as_ref(), x, y - 14, 2, DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER, None, None);
            }
            if g.unit_selected == Some(u) {
                let s = self.sprite(6).cloned();
                self.screen.draw_sprite(s.as_ref(), x, y, 2, DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER, None, None);
            }
        }

        // Explosions
        for e in g.explosions.clone() {
            if e.commands.is_none() || e.sprite_id == 0 {
                continue;
            }
            if !g.map[e.position.pack() as usize].is_unveiled {
                continue;
            }
            let (ok, x, y) = g.map_is_position_in_viewport(e.position);
            if !ok {
                continue;
            }
            let s = self.sprite(e.sprite_id).cloned();
            self.screen.draw_sprite(s.as_ref(), x as i32, y as i32, 2, DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER, None, None);
        }

        // Air units
        for &u in &g.unit_find.clone() {
            const VALUES_32E4: [[u16; 2]; 8] = [[0, 0], [1, 0], [2, 0], [1, 2], [0, 2], [1, 3], [2, 1], [1, 1]];
            let un = g.u(u);
            if un.o.index > 15 || un.o.flags.is_not_on_map {
                continue;
            }
            if !g.map[un.o.position.pack() as usize].is_unveiled {
                continue;
            }
            let ui = *g.ui(u);
            let (ok, x, y) = g.map_is_position_in_viewport(un.o.position);
            if !ok {
                continue;
            }
            let (x, y) = (x as i32, y as i32);
            let mut index = ui.ground_sprite_id;
            let o = un.orientation[0].current as u8;
            let mut sprite_flags = DRAWSPRITE_FLAG_WIDGETPOS | DRAWSPRITE_FLAG_CENTER;
            match ui.display_mode {
                DISPLAYMODE_SINGLE_FRAME => {
                    if un.o.flags.bullet_is_big {
                        index += 1;
                    }
                }
                DISPLAYMODE_UNIT => {
                    let o8 = orientation_256_to_8(o) as usize;
                    index += VALUES_32E4[o8][0];
                    sprite_flags |= VALUES_32E4[o8][1];
                }
                DISPLAYMODE_ROCKET => {
                    const V: [[u16; 2]; 16] = [[0, 0], [1, 0], [2, 0], [3, 0], [4, 0], [3, 2], [2, 2], [1, 2], [0, 2], [3, 3], [2, 3], [3, 3], [4, 1], [3, 1], [2, 1], [1, 1]];
                    let o16 = orientation_256_to_16(o) as usize;
                    index += V[o16][0];
                    sprite_flags |= V[o16][1];
                }
                DISPLAYMODE_ORNITHOPTER => {
                    const V: [u16; 4] = [2, 1, 0, 1];
                    let o8 = orientation_256_to_8(o) as usize;
                    index += VALUES_32E4[o8][0] * 3 + V[(un.sprite_offset & 3) as usize];
                    sprite_flags |= VALUES_32E4[o8][1];
                }
                _ => sprite_flags = 0,
            }
            if ui.flags.has_animation_set && un.o.flags.animation_flip {
                index += 5;
            }
            if un.o.type_ == UNIT_CARRYALL && un.o.flags.in_transport {
                index += 3;
            }
            let s = self.sprite(index).cloned();
            if ui.o.flags.has_shadow {
                self.screen.draw_sprite(s.as_ref(), x + 1, y + 3, 2, (sprite_flags & !DRAWSPRITE_FLAG_PAL) | DRAWSPRITE_FLAG_REMAP | DRAWSPRITE_FLAG_BLUR, None, Some((&self.pal_map1, 1)));
            }
            if ui.o.flags.blur_tile {
                sprite_flags |= DRAWSPRITE_FLAG_BLUR;
            }
            match s.as_ref().and_then(|sp| Self::house_palette(sp, g.unit_get_house_id(u))) {
                Some(pal) => self.screen.draw_sprite(s.as_ref(), x, y, 2, sprite_flags | DRAWSPRITE_FLAG_PAL, Some(&pal), None),
                None => self.screen.draw_sprite(s.as_ref(), x, y, 2, sprite_flags, None, None),
            }
        }
        self.screen.clip = Clip { left: 0, top: 0, right: SCREEN_WIDTH as i16 - 1, bottom: SCREEN_HEIGHT as i16 - 1 };
    }

    /// GUI_Widget_Viewport_DrawTile for every tile + Map_UpdateMinimapPosition.
    fn draw_minimap(&mut self) {
        let g = &self.game;
        let scale = g.scenario.map_scale;
        let (min_x, min_y, _, _) = g.map_info();
        let ph = g.player_house_id;
        let radar = g.h(ph).flags.radar_activated || self.debug_full_radar;
        for packed in 0..4096u16 {
            if !g.map_is_valid_position(packed) {
                continue;
            }
            let mut colour: u16 = 12;
            let mut sprite_id: u16 = 0xFFFF;
            let ms = scale + 1;
            if g.map[packed as usize].is_unveiled && radar {
                let t = g.map_get_landscape_type(packed);
                if ms > 1 {
                    sprite_id = scale + LANDSCAPE_INFO[t as usize].sprite_id - 1;
                } else {
                    colour = LANDSCAPE_INFO[t as usize].radar_colour;
                }
                if LANDSCAPE_INFO[t as usize].radar_colour == 0xFFFF {
                    let h = g.map[packed as usize].house_id;
                    if ms > 1 {
                        sprite_id = ms + h as u16 * 2 + 29;
                    } else {
                        colour = HOUSE_INFO[h as usize].minimap_color;
                    }
                }
                if let Some(u) = g.unit_get_by_packed_tile(packed) {
                    if ms > 1 {
                        sprite_id = if g.u(u).o.type_ == UNIT_SANDWORM { ms + 53 } else { ms + g.unit_get_house_id(u) as u16 * 2 + 29 };
                    } else {
                        colour = if g.u(u).o.type_ == UNIT_SANDWORM { 255 } else { HOUSE_INFO[g.unit_get_house_id(u) as usize].minimap_color };
                    }
                }
            } else {
                match g.structure_get_by_packed_tile(packed) {
                    Some(s) if g.s(s).o.house_id == ph => {
                        if ms > 1 {
                            sprite_id = ms + ph as u16 * 2 + 29;
                        } else {
                            colour = HOUSE_INFO[ph as usize].minimap_color;
                        }
                    }
                    _ => {
                        if ms > 1 {
                            sprite_id = scale + LANDSCAPE_INFO[LST_ENTIRELY_MOUNTAIN as usize].sprite_id - 1;
                        } else {
                            colour = 12;
                        }
                    }
                }
            }
            let x = (packed_x(packed) - min_x) as i32;
            let y = (packed_y(packed) - min_y) as i32;
            if sprite_id != 0xFFFF {
                let s = self.sprite(sprite_id).cloned();
                self.screen.draw_sprite(s.as_ref(), x * ms as i32, y * ms as i32, 3, DRAWSPRITE_FLAG_WIDGETPOS, None, None);
            } else {
                self.screen.put_pixel(x + 256, y + 136, (colour & 0xFF) as u8);
            }
        }
        // Viewport rectangle
        let p = g.minimap_position;
        let ms = scale as i32;
        let left = (packed_x(p) as i32 - min_x as i32) * (ms + 1) + 256;
        let right = left + ms * 15 + 14;
        let top = (packed_y(p) as i32 - min_y as i32) * (ms + 1) + 136;
        let bottom = top + ms * 10 + 9;
        self.screen.wired_rect(left, top, right, bottom, 15);
    }

    /// GUI_UpdateProductionStringID
    fn update_production_string_id(&mut self) {
        self.production_string_id = STR_NULL;
        let g = &self.game;
        let Some(s) = g.structure_get_by_packed_tile(g.selection_position) else { return };
        let st = g.s(s);
        if !g.si(s).o.flags.factory {
            if st.o.type_ == STRUCTURE_PALACE {
                self.production_string_id = STR_LAUNCH + HOUSE_INFO[st.o.house_id as usize].special_weapon - 1;
            }
            return;
        }
        self.production_string_id = if st.o.flags.upgrading {
            STR_UPGRADINGD_DONE
        } else if st.o.linked_id == 0xFF {
            STR_BUILD_IT
        } else if st.o.flags.on_hold {
            STR_ON_HOLD
        } else if st.count_down != 0 {
            STR_D_DONE
        } else if st.o.type_ == STRUCTURE_CONSTRUCTION_YARD {
            STR_PLACE_IT
        } else {
            STR_COMPLETED
        };
    }

    /// GUI_Widget_ActionPanel_Draw (always force-drawn).
    fn draw_action_panel(&mut self) {
        let g = &self.game;
        let action_type: u16 = if g.selection_type == SelectionType::Place {
            7
        } else if g.unit_house_missile.is_some() {
            8
        } else if g.unit_selected.is_some() {
            if g.selection_type == SelectionType::Target {
                match g.active_action as u8 {
                    ACTION_ATTACK => 4,
                    ACTION_MOVE => 5,
                    _ => 6,
                }
            } else {
                2
            }
        } else if !crate::tile::is_out_of_map(g.selection_position) && g.map[g.selection_position as usize].is_unveiled && g.map_get_landscape_type(g.selection_position) == LST_STRUCTURE {
            3
        } else {
            1
        };

        let mut obj: Option<ObjRef> = None;
        let mut oi = None;
        let mut not_player = false;
        let mut house = g.player_house_id;
        match action_type {
            2 => {
                let u = g.unit_selected.unwrap();
                obj = Some(ObjRef::Unit(u));
                oi = Some(g.ui(u).o);
                not_player = g.player_house_id != g.unit_get_house_id(u);
                house = g.u(u).o.house_id;
            }
            3 => {
                let s = g.structure_get_by_packed_tile(g.selection_position).unwrap();
                obj = Some(ObjRef::Structure(s));
                oi = Some(g.si(s).o);
                not_player = g.player_house_id != g.s(s).o.house_id;
                house = g.s(s).o.house_id;
                self.structure_high_health = g.s(s).o.hitpoints > g.si(s).o.hitpoints / 2;
                if g.s(s).upgrade_time_left == 0 && g.structure_is_upgradable(s) {
                    self.game.sm(s).upgrade_time_left = 100;
                }
                self.update_production_string_id();
            }
            7 => {
                oi = Some(g.structure_info[g.structure_active_type as usize].o);
            }
            8 => {
                let u = g.unit_house_missile.unwrap();
                obj = Some(ObjRef::Unit(u));
                oi = Some(g.ui(u).o);
            }
            _ => {}
        }

        for i in [7u16, 4, 6, 5, 3, 8, 9, 10, 11] {
            self.widgets.make_visible(i, false);
        }
        self.screen.widget_border(6, 0, false);

        if action_type > 1 {
            let fonts = &self.fonts;
            let string_id = match action_type {
                4 => STR_TARGET,
                5 => STR_MOVEMENT,
                6 => STR_HARVEST,
                _ => oi.map(|o| o.string_id_abbrev).unwrap_or(STR_NULL),
            };
            if string_id != STR_NULL {
                let t = self.game.string(string_id);
                self.screen.draw_text_wrapper(fonts, &t, 288, 43, 29, 0, 0x111);
            }
            let sprite_id = match action_type {
                3 if oi.unwrap().flags.factory && !not_player => {
                    self.widgets.make_visible(6, true);
                    0xFFFF
                }
                2 | 3 | 7 => oi.unwrap().sprite_id,
                5 | 6 => 0x1D,
                4 => 0x1C,
                8 => 0x1E,
                _ => 0xFFFF,
            };
            if sprite_id != 0xFFFF {
                let s = self.sprite(sprite_id).cloned();
                self.screen.draw_sprite(s.as_ref(), 258, 51, 0, 0, None, None);
            }
            if action_type == 2 || action_type == 3 {
                let hp = self.game.obj(obj.unwrap()).hitpoints;
                self.draw_progressbar(hp, oi.unwrap().hitpoints);
                let s = self.sprite(27).cloned();
                self.screen.draw_sprite(s.as_ref(), 292, 60, 0, 0, None, None);
                let t = self.game.string(STR_DMG);
                self.screen.draw_text_wrapper(&self.fonts, &t, 296, 65, 29, 0, 0x11);
            }
            if !not_player {
                self.draw_action_panel_details(action_type, obj, oi, house);
            }
        }
    }

    fn draw_action_panel_details(&mut self, action_type: u16, obj: Option<ObjRef>, oi: Option<crate::tables::ObjectInfo>, house: u8) {
        let blink = WIDGET_PROPERTIES[6].4;
        match action_type {
            2 => {
                let ObjRef::Unit(u) = obj.unwrap() else { return };
                self.widgets.make_visible(3, true);
                let un = self.game.u(u);
                let current = if un.next_action_id != ACTION_INVALID { un.next_action_id } else { un.action_id };
                let actions = oi.unwrap().actions_player;
                for i in 0..4 {
                    let sid = ACTION_INFO[actions[i] as usize].string_id;
                    let first = *self.game.strings.get(sid).first().unwrap_or(&0);
                    if let Some(w) = self.widgets.get_mut(i as u16 + 8) {
                        w.string_id = sid;
                        w.shortcut = get_shortcut(first);
                        w.invisible = false;
                        w.selected = actions[i] as u8 == current;
                    }
                }
            }
            3 => {
                let ObjRef::Structure(s) = obj.unwrap() else { return };
                let oi = oi.unwrap();
                self.widgets.make_visible(3, true);
                let st = self.game.s(s).clone();
                let repair_string = if st.o.flags.upgrading {
                    Some((STR_UPGRADING, true))
                } else if st.o.hitpoints != oi.hitpoints {
                    Some(if st.o.flags.repairing { (STR_REPAIRING, true) } else { (STR_REPAIR, false) })
                } else if st.upgrade_time_left != 0 {
                    Some((STR_UPGRADE, false))
                } else {
                    None
                };
                if let Some((sid, sel)) = repair_string {
                    if let Some(w) = self.widgets.get_mut(4) {
                        w.string_id = sid;
                        w.invisible = false;
                        w.selected = sel;
                    }
                }
                if st.o.type_ != STRUCTURE_STARPORT && (oi.flags.factory || (st.o.type_ == STRUCTURE_PALACE && st.count_down == 0)) {
                    self.widgets.make_visible(5, true);
                }
                let si = *self.game.si(s);
                let h = self.game.h(house).clone();
                let fonts = &self.fonts;
                match st.o.type_ {
                    STRUCTURE_REPAIR => {
                        if st.o.linked_id != 0xFF {
                            let lt = self.game.u(st.o.linked_id as u16).o.type_ as usize;
                            let s2 = self.sprite(self.game.unit_info[lt].o.sprite_id).cloned();
                            self.screen.draw_sprite(s2.as_ref(), 260, 89, 0, 0, None, None);
                            let steps = self.game.unit_info[lt].o.build_time / 4;
                            let percent = (steps as i32 - (st.count_down >> 8) as i32) * 100 / steps.max(1) as i32;
                            let t = format_c(&self.game.string(STR_D_DONE), &[Arg::D(percent)]);
                            self.screen.draw_text_wrapper(fonts, &t, 258, 116, 29, 0, 0x11);
                        }
                    }
                    STRUCTURE_WINDTRAP => {
                        let output = (st.o.hitpoints as i32 * -si.power_usage as i32 / oi.hitpoints as i32) as u16;
                        let average = if h.windtrap_count == 0 { 0 } else { h.power_usage / h.windtrap_count };
                        self.screen.line(261, 95, 312, 95, 16);
                        let t = self.game.string(STR_POWER_INFONEEDEDOUTPUT);
                        self.screen.draw_text_wrapper(fonts, &t, 258, 88, 29, 0, 0x11);
                        let fh = self.screen.font_height(fonts);
                        let a = format!("{average:3}").into_bytes();
                        self.screen.draw_text_wrapper(fonts, &a, 292, fh * 2 + 80, 29, 0, 0x11);
                        let o = format!("{output:3}").into_bytes();
                        self.screen.draw_text_wrapper(fonts, &o, 292, fh * 3 + 80, if output >= average { 29 } else { 6 }, 0, 0x11);
                    }
                    STRUCTURE_STARPORT => {
                        let t = if h.starport_linked_id != 0xFFFF {
                            format_c(&self.game.string(STR_FRIGATEARRIVAL_INTMINUS_D), &[Arg::D(h.starport_time_left as i32)])
                        } else {
                            self.game.string(STR_FRIGATE_INORBIT_ANDAWAITINGORDER)
                        };
                        self.screen.draw_text_wrapper(fonts, &t, 258, 88, 29, 0, 0x11);
                    }
                    STRUCTURE_REFINERY | STRUCTURE_SILO => {
                        let mut stored = if h.credits_storage == 0 { 0 } else { (h.credits as u32 * si.credits_storage as u32 / h.credits_storage as u32) as i32 };
                        if h.credits > h.credits_storage {
                            stored = si.credits_storage as i32;
                        }
                        self.screen.line(261, 95, 312, 95, 16);
                        let t = format_c(&self.game.string(STR_SPICEHOLDS_4DMAX_4D), &[Arg::D(stored), Arg::D(si.credits_storage.min(1000) as i32)]);
                        self.screen.draw_text_wrapper(fonts, &t, 258, 88, 29, 0, 0x11);
                    }
                    STRUCTURE_OUTPOST => {
                        self.screen.line(261, 95, 312, 95, 16);
                        let t = format_c(&self.game.string(STR_RADAR_SCANFRIEND_2DENEMY_2D), &[Arg::D(h.unit_count_allied as i32), Arg::D(h.unit_count_enemy as i32)]);
                        self.screen.draw_text_wrapper(fonts, &t, 258, 88, 29, 0, 0x11);
                    }
                    _ => {}
                }
            }
            4 | 5 | 6 | 7 => {
                self.widgets.make_visible(7, true);
                let (sid, y) = match action_type {
                    4 => (STR_SELECTTARGET, 76),
                    5 => (STR_SELECTDESTINATION, 76),
                    6 => (STR_SELECTPLACE_TOHARVEST, 76),
                    _ => (STR_SELECTLOCATION_TOBUILD, 84),
                };
                let t = self.game.string(sid);
                self.screen.draw_text_wrapper(&self.fonts, &t, 259, y, blink, 0, 0x11);
            }
            8 => {
                let count = (self.game.house_missile_countdown as i32 - 1).max(0);
                let t = format_c(&self.game.string(STR_PICK_TARGETTMINUS_D), &[Arg::D(count)]);
                self.screen.draw_text_wrapper(&self.fonts, &t, 259, 84, blink, 0, 0x11);
            }
            _ => {}
        }
    }

    /// GUI_DrawProgressbar
    fn draw_progressbar(&mut self, current: u16, max: u16) {
        const INFO: [i32; 11] = [293, 52, 24, 7, 1, 0, 0, 0, 4, 5, 8];
        let current = current.min(max) as i32;
        let max = (max as i32).max(1);
        let width = INFO[2];
        let mut height = INFO[3];
        height = (current * height / max).max(1);
        let _ = width;
        let mut colour = INFO[8];
        if current <= max / 2 {
            colour = INFO[9];
        }
        if current <= max / 4 {
            colour = INFO[10];
        }
        self.screen.border(INFO[0] - 1, INFO[1] - 1, INFO[2] + 2, INFO[3] + 2, 1, true);
        self.screen.fill_rect(INFO[0], INFO[1] + INFO[3] - height, INFO[0] + INFO[2] - 1, INFO[1] + INFO[3] - 1, colour as u8);
    }

    /// GUI_Widget_Draw for all visible widgets.
    fn draw_widgets(&mut self) {
        let list = self.widgets.list.clone();
        for w in list {
            if w.invisible {
                if w.grey_when_invisible {
                    let (x, y) = w.position();
                    self.screen.blocked_rect(x, y, w.width as i32, w.height as i32, 12);
                }
                continue;
            }
            match w.kind {
                WidgetKind::Sprite { normal, down } => {
                    let id = if w.hover2 { down } else { normal };
                    let s = self.sprite(id).cloned();
                    self.screen.draw_sprite(s.as_ref(), w.offset_x as i32, w.offset_y as i32, w.parent_id, DRAWSPRITE_FLAG_REMAP | DRAWSPRITE_FLAG_WIDGETPOS, None, Some((&self.remap, 1)));
                }
                WidgetKind::SpriteButton => self.draw_sprite_button(&w),
                WidgetKind::SpriteTextButton => self.draw_sprite_text_button(&w),
                WidgetKind::TextButton2 => self.draw_text_button2(&w),
                WidgetKind::None => {}
            }
        }
    }

    /// GUI_Widget_SpriteButton_Draw (the picture of the selected object)
    fn draw_sprite_button(&mut self, w: &Widget) {
        let g = &self.game;
        let sprite_id = if let Some(u) = g.unit_selected {
            g.ui(u).o.sprite_id
        } else {
            let Some(s) = g.structure_get_by_packed_tile(g.selection_position) else { return };
            g.si(s).o.sprite_id
        };
        let (x, y, wd, h) = (w.offset_x as i32, w.offset_y as i32, w.width as i32, w.height as i32);
        self.screen.wired_rect(x - 1, y - 1, x + wd, y + h, 12);
        let s = self.sprite(sprite_id).cloned();
        self.screen.draw_sprite(s.as_ref(), x, y, 0, DRAWSPRITE_FLAG_REMAP, None, Some((&self.pal_map1, if w.hover2 { 1 } else { 0 })));
        self.screen.border(x, y, wd, h, if w.hover2 { 0 } else { 1 }, false);
    }

    /// GUI_Widget_SpriteTextButton_Draw (Build It / Place It / ...)
    fn draw_sprite_text_button(&mut self, w: &Widget) {
        let Some(s) = self.game.structure_get_by_packed_tile(self.game.selection_position) else { return };
        self.update_production_string_id();
        let down = w.hover2;
        let (x, y, wd, h) = (w.offset_x as i32, w.offset_y as i32, w.width as i32, w.height as i32);
        self.screen.wired_rect(x - 1, y - 1, x + wd, y + h, 12);
        self.screen.border(x, y, wd, h, if down { 0 } else { 1 }, true);
        let st = self.game.s(s).clone();
        let mut sprite_id: u16 = 0;
        match self.production_string_id {
            STR_LAUNCH => sprite_id = 0x1E,
            STR_FREMEN => sprite_id = 0x5E,
            STR_SABOTEUR => sprite_id = 0x60,
            STR_PLACE_IT | STR_COMPLETED | STR_ON_HOLD | STR_BUILD_IT | STR_D_DONE => {
                if st.o.type_ == STRUCTURE_CONSTRUCTION_YARD {
                    let s63 = self.sprite(63).cloned();
                    self.screen.draw_sprite(s63.as_ref(), x + 37, y + 5, 0, DRAWSPRITE_FLAG_REMAP, None, Some((&self.pal_map1, if down { 2 } else { 0 })));
                    let s24 = self.sprite(24).cloned();
                    let sw = s24.as_ref().map(|s| s.width as i32).unwrap_or(0) + 1;
                    let si = self.game.structure_info[st.object_type as usize % STRUCTURE_MAX];
                    let (lw, lh) = STRUCTURE_LAYOUT_SIZE[si.layout as usize];
                    for yy in 0..lh as i32 {
                        for xx in 0..lw as i32 {
                            self.screen.draw_sprite(s24.as_ref(), x + xx * sw + 38, y + yy * sw + 6, 0, 0, None, None);
                        }
                    }
                    sprite_id = si.o.sprite_id;
                } else {
                    sprite_id = self.game.unit_info[st.object_type as usize % UNIT_MAX].o.sprite_id;
                }
            }
            _ => {}
        }
        if sprite_id != 0 {
            let sp = self.sprite(sprite_id).cloned();
            self.screen.draw_sprite(sp.as_ref(), x + 2, y + 2, 0, DRAWSPRITE_FLAG_REMAP, None, Some((&self.pal_map1, if down { 1 } else { 0 })));
        }
        let mut percent: i32 = 0;
        if self.production_string_id == STR_D_DONE {
            let build_time = if st.o.type_ == STRUCTURE_CONSTRUCTION_YARD {
                self.game.structure_info[st.object_type as usize].o.build_time
            } else if st.o.type_ == STRUCTURE_REPAIR {
                if st.o.linked_id == 0xFF {
                    return;
                }
                self.game.ui(st.o.linked_id as u16).o.build_time
            } else {
                self.game.unit_info[st.object_type as usize].o.build_time
            } as i32;
            let time_left = build_time - (st.count_down as i32 + 255) / 256;
            percent = 100 * time_left / build_time.max(1);
        }
        let fmt = self.game.string(self.production_string_id);
        if self.production_string_id == STR_UPGRADINGD_DONE {
            percent = 100 - st.upgrade_time_left as i32;
            let t = format_c(&fmt, &[Arg::D(percent)]);
            self.screen.draw_text_wrapper(&self.fonts, &t, x + 1, y + h - 19, if down { 0xE } else { 0xF }, 0, 0x021);
        } else {
            let t = format_c(&fmt, &[Arg::D(percent)]);
            let c = if self.production_string_id == STR_PLACE_IT { 0xEF } else if down { 0xE } else { 0xF };
            self.screen.draw_text_wrapper(&self.fonts, &t, x + wd / 2, y + h - 9, c, 0, 0x121);
        }
        let sc_id = if self.production_string_id == STR_D_DONE || self.production_string_id == STR_UPGRADINGD_DONE { STR_ON_HOLD } else { self.production_string_id };
        let first = *self.game.strings.get(sc_id).first().unwrap_or(&0);
        if let Some(wm) = self.widgets.get_mut(w.index) {
            wm.shortcut = get_shortcut(first);
        }
    }

    /// GUI_Widget_TextButton2_Draw
    fn draw_text_button2(&mut self, w: &Widget) {
        let (x, y, wd, h) = (w.offset_x as i32, w.offset_y as i32, w.width as i32, w.height as i32);
        self.screen.wired_rect(x - 1, y - 1, x + wd, y + h, 12);
        self.screen.border(x, y, wd, h, if w.hover2 { 0 } else { 1 }, true);
        let mut colour = 0xF;
        if w.selected {
            colour = 0x6;
        } else if w.hover2 {
            colour = 0xE;
        }
        if !w.hover2 && w.string_id == STR_REPAIR {
            colour = 0xEF;
        }
        let t = self.game.string(w.string_id);
        self.screen.draw_text_wrapper(&self.fonts, &t, x + wd / 2, y + 1, colour, 0, 0x121);
        let first = *t.first().unwrap_or(&0);
        if let Some(wm) = self.widgets.get_mut(w.index) {
            wm.shortcut = get_shortcut(first);
        }
    }

    /// Convert the indexed screen to RGBA with the current (animated) palette.
    pub fn to_rgba(&self, out: &mut [u8]) {
        let pal = crate::data::image::Palette(self.palette).to_rgba();
        for (i, &p) in self.screen.pixels.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&pal[p as usize]);
        }
    }
}
