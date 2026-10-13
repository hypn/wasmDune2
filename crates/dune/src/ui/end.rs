//! The end of a skirmish (GameLoop_LevelEnd, as far as it applies to a skirmish).
//!
//! As in the original: the music stops, the win or lose voice plays, the modal message says
//! whether the mission was completed or failed, and then GUI_EndStats_Show: the score, time and
//! rank, and bars for spice harvested, units destroyed and buildings destroyed, drawn on
//! FAME.CPS one pixel per tick. The original only showed the stats after a win (a lost mission
//! went to the Mentat); here they follow both. The shareware data has no FAME.CPS, so the screen
//! is then drawn in the same layout from SCREEN.CPS's sand.
//!
//! Afterwards (new: the original went on to the Hall of Fame and the next mission) a window over
//! the shaded game offers to restart the same map, to go back to the setup screen, or to carry
//! on playing.

use super::screen::*;
use super::setup::window_ink;
use super::{GameUi, Modal, SoundRequest};
use crate::game::*;
use crate::tables::strings::*;

/// _rankScores in DUNE2.EXE (v1.07): (string, lowest score). OpenDUNE's table has two more
/// ranks, which the game's text files never had.
const RANK_SCORES: [(u16, u16); 10] = [
    (271, 25),   // Sand Flea
    (272, 50),   // Sand Snake
    (273, 100),  // Desert Mongoose
    (274, 150),  // Sand Warrior
    (275, 200),  // Dune Trooper
    (276, 300),  // Squad Leader
    (277, 400),  // Outpost Commander
    (278, 500),  // Base Commander
    (279, 700),  // Scourge of Dune
    (280, 1000), // Ruler of Arrakis
];

// The end window, in screen pixels.
const WIN_X: i32 = 60;
const WIN_Y: i32 = 50;
const WIN_W: i32 = 200;
const WIN_H: i32 = 86;
const BUTTON_W: i32 = 136;
const BUTTON_H: i32 = 15;
const BUTTON_X: i32 = (WIN_W - BUTTON_W) / 2;
/// The buttons' offsets from the top of the window.
const BUTTON_Y: [i32; 3] = [26, 43, 60];
const RESTART: usize = 0;
const NEW_GAME: usize = 1;
const CONTINUE: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EndStage {
    /// The modal message is up.
    Message,
    /// The stats screen (GUI_EndStats_Show).
    Stats,
    /// The end window is up (the game stays paused).
    Window,
    /// "Continue game" was chosen: the game runs on.
    Dismissed,
}

/// One step of GUI_EndStats_Show's drawing, done as time passes.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// GUI_EndStats_Sleep(ticks)
    Sleep(u16),
    /// GUI_HallOfFame_DrawRank
    Rank,
    /// One more pixel of a bar, with the count so far.
    Bar { x: i32, y: i32, colour: u8, count: u16 },
    /// The bar's final count.
    BarEnd { y: i32, value: u16 },
}

pub struct EndStats {
    /// SCREEN_1: the stats screen being drawn.
    screen: Screen,
    steps: Vec<Step>,
    next: usize,
    wait: u16,
    score: u16,
    /// GUI_HallOfFame_Tick: palette entry 255 (the player's bars) pulses in one channel.
    pal_channel: usize,
    pal_direction: i16,
    pal_timer: u32,
    pal_backup: [u8; 3],
    ticks: u32,
}

pub struct EndScreen {
    pub won: bool,
    pub stage: EndStage,
    stats: Option<EndStats>,
    /// The end window's button held down with the left mouse button.
    pressed: Option<usize>,
}

fn button_at(x: i32, y: i32) -> Option<usize> {
    let (x, y) = (x - WIN_X, y - WIN_Y);
    if !(BUTTON_X..BUTTON_X + BUTTON_W).contains(&x) {
        return None;
    }
    BUTTON_Y.iter().position(|&by| y >= by && y < by + BUTTON_H)
}

/// GUI_HallOfFame_GetRank
fn rank(score: u16) -> u16 {
    let i = RANK_SCORES.iter().position(|r| r.1 > score).unwrap_or(RANK_SCORES.len());
    RANK_SCORES[i.min(RANK_SCORES.len() - 1)].0
}

/// GUI_Screen_Copy / GFX_Screen_Copy2 within one screen.
fn copy_area(s: &mut Screen, sx: i32, sy: i32, dx: i32, dy: i32, w: i32, h: i32) {
    for y in 0..h {
        let src = ((sy + y) * SCREEN_WIDTH as i32 + sx) as usize;
        let dst = ((dy + y) * SCREEN_WIDTH as i32 + dx) as usize;
        s.pixels.copy_within(src..src + w as usize, dst);
    }
}

/// What the end of the game plays (see `audio_tick`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EndMusic {
    /// Music_Play(0)
    Silence,
    /// GUI_EndStats_Show's Music_Play(17 + Tools_RandomLCG_Range(0, 5))
    Stats,
}

impl GameUi {
    /// The music the end of the game wants instead of the in-game rotation.
    pub(super) fn end_music(&self) -> Option<EndMusic> {
        match self.end.as_ref()?.stage {
            EndStage::Stats => Some(EndMusic::Stats),
            EndStage::Dismissed => None,
            _ => Some(EndMusic::Silence),
        }
    }

    /// Start the ending once the game has decided it (Game::skirmish_check_end).
    pub(super) fn end_check(&mut self) {
        let Some(won) = self.game.skirmish.finished else { return };
        if self.end.is_some() {
            return;
        }
        // GameLoop_LevelEnd: Music_Play(0) (see audio_tick), the arrow cursor, then
        // GUI_DisplayModalMessage. The voice was queued by the game.
        self.cursor_sprite = 0;
        self.cursor_hotspot = (0, 0);
        let string_id = if won { STR_YOU_HAVE_SUCCESSFULLY_COMPLETED_YOUR_MISSION } else { STR_YOU_HAVE_FAILED_YOUR_MISSION };
        let text = self.game.string(string_id);
        self.modal = Some(Modal { text, sprite: 0xFFFF, wait: 30 });
        self.end = Some(EndScreen { won, stage: EndStage::Message, stats: None, pressed: None });
    }

    /// True while the end of the game has the game paused (after the modal message): one tick
    /// of the stats screen or the end window.
    pub(super) fn end_tick(&mut self) -> bool {
        let Some(mut e) = self.end.take() else { return false };
        if e.stage == EndStage::Message && self.modal.is_none() {
            e.stage = EndStage::Stats;
            e.stats = Some(self.end_stats_begin());
            self.input.queue.clear();
        }
        let paused = match e.stage {
            EndStage::Stats => {
                let mut st = e.stats.take().unwrap();
                if self.end_stats_tick(&mut st) {
                    self.palette[765..768].copy_from_slice(&st.pal_backup);
                    e.stage = EndStage::Window;
                } else {
                    e.stats = Some(st);
                }
                true
            }
            EndStage::Window => {
                if let Some(choice) = self.end_window_input(&mut e) {
                    match choice {
                        RESTART | NEW_GAME => {
                            self.setup_reopen(choice == RESTART);
                            return true;
                        }
                        _ => e.stage = EndStage::Dismissed,
                    }
                }
                true
            }
            _ => false,
        };
        self.end = Some(e);
        paused && self.end.as_ref().is_some_and(|e| e.stage != EndStage::Dismissed)
    }

    /// GUI_EndStats_Show up to its animation: the score (Update_Score), the background
    /// (GUI_HallOfFame_DrawBackground), the captions, and the drawing steps.
    fn end_stats_begin(&mut self) -> EndStats {
        let g = &self.game;
        let ph = g.player_house_id;
        let ticks_played = (g.timer_game.wrapping_sub(g.tick_scenario_start) / 3600 + 1) as u16;

        // Update_Score
        let mut score = (g.scenario.score as i16).max(0) as u16;
        for s in g.structures_of(ph, 0xFFFF) {
            let t = g.s(s).o.type_;
            if matches!(t, STRUCTURE_SLAB_1X1 | STRUCTURE_SLAB_2X2 | STRUCTURE_WALL) {
                continue;
            }
            score = score.wrapping_add(g.structure_info[t as usize].o.build_credits / 100);
        }
        let (mut sum_allied, mut sum_enemy) = (0u16, 0u16);
        for u in g.units_of(HOUSE_INVALID, UNIT_HARVESTER as u16) {
            let amount = (g.u(u).amount as u16).wrapping_mul(7);
            if g.house_are_allied(g.unit_get_house_id(u), ph) {
                sum_allied = sum_allied.wrapping_add(amount);
            } else {
                sum_enemy = sum_enemy.wrapping_add(amount);
            }
        }
        let harvested_enemy = (g.scenario.harvested_enemy as u32 + sum_enemy as u32).min(65000) as u16;
        let harvested_allied = (g.scenario.harvested_allied as u32 + sum_allied as u32).min(65000) as u16;
        score = score.wrapping_add(g.h(ph).credits / 100);
        score = (score as i16).max(0) as u16;
        let target_time = g.campaign_id.wrapping_mul(45);
        if ticks_played < target_time {
            score = score.wrapping_add(target_time - ticks_played);
        }

        // GUI_HallOfFame_DrawBackground
        let mut s = Screen::new();
        if let Some(cps) = self.assets.vfs.get("FAME.CPS") {
            s.pixels = crate::data::image::load_cps(cps).pixels;
            let mut x_src = if ph <= HOUSE_ORDOS { (ph as i32 * 56 + 8) / 8 } else { 1 };
            copy_area(&mut s, x_src * 8, 136, 0, 8, 56, 56);
            if ph > HOUSE_ORDOS {
                x_src += 7;
            }
            copy_area(&mut s, x_src * 8, 136, 33 * 8, 8, 56, 56);
            s.fill_rect(8, 136, 175, 191, 116);
            copy_area(&mut s, 8, 80, 8, 116, 304, 36);
            copy_area(&mut s, 8, 80, 8, 152, 304, 36);
        } else {
            s.pixels = self.end_stats_fallback_background();
        }
        let fonts = &self.fonts;
        let mut time = format_c(&self.game.string(STR_TIME_DH_DM), &[Arg::D(ticks_played as i32 / 60), Arg::D(ticks_played as i32 % 60)]);
        if ticks_played < 60 {
            // Drop the "0h" part.
            if let Some(i) = time.iter().position(|&c| c == b'0') {
                while i < time.len() && time[i] != b' ' {
                    time.remove(i);
                }
            }
        }
        let score_text = format_c(&self.game.string(STR_SCORE_D), &[Arg::D(score as i32)]);
        s.draw_text_wrapper(fonts, &score_text, 72, 15, 15, 0, 0x22);
        s.draw_text_wrapper(fonts, &time, 248, 15, 15, 0, 0x222);
        s.draw_text_wrapper(fonts, &self.game.string(STR_YOU_HAVE_ATTAINED_THE_RANK_OF), SCREEN_WIDTH as i32 / 2, 38, 15, 0, 0x122);
        let (colour, offset) = match ph {
            HOUSE_HARKONNEN => (149, 0),
            HOUSE_ORDOS => (181, 1),
            _ => (165, 2),
        };
        let pal_backup = [self.palette[765], self.palette[766], self.palette[767]];
        self.palette.copy_within(colour * 3..colour * 3 + 3, 765);

        // GUI_DrawTextOnFilledRectangle
        for (string_id, top) in [(STR_SPICE_HARVESTED_BY, 83), (STR_UNITS_DESTROYED_BY, 119), (STR_BUILDINGS_DESTROYED_BY, 155)] {
            let t = self.game.string(string_id);
            s.draw_text_wrapper(fonts, b"", 0, 0, 0, 0, 0x121);
            let half = s.string_width(fonts, &t) / 2 + 4;
            let mid = SCREEN_WIDTH as i32 / 2;
            s.fill_rect(mid - half, top, mid + half, top + 6, 116);
            s.draw_text_wrapper(fonts, &t, mid, top, 0xF, 0, 0x121);
        }
        let (you, enemy) = (self.game.string(STR_YOU), self.game.string(STR_ENEMY));
        let text_left = 19 + s.string_width(fonts, &you).max(s.string_width(fonts, &enemy));
        let bar_width = 261 - text_left;
        for i in 0..3 {
            s.draw_text_wrapper(fonts, &you, text_left - 4, 92 + i * 36, 0xF, 0, 0x221);
            s.draw_text_wrapper(fonts, &enemy, text_left - 4, 101 + i * 36, 0xF, 0, 0x221);
        }

        let sc = &self.game.scenario;
        let scores = [[harvested_allied, harvested_enemy], [sc.killed_enemy, sc.killed_allied], [sc.destroyed_enemy, sc.destroyed_allied]];
        let mut steps = vec![Step::Sleep(45), Step::Rank, Step::Sleep(45)];
        for (i, pair) in scores.iter().enumerate() {
            let pair = pair.map(|v| v.min(65000));
            let increment = 1 + pair[0].max(pair[1]) / bar_width as u16;
            for (j, &value) in pair.iter().enumerate() {
                let y = 93 + i as i32 * 36 + j as i32 * 9;
                let colour = if j == 0 { 255 } else { 209 };
                let (mut x, mut count) = (text_left, 0u16);
                while count < value {
                    steps.push(Step::Bar { x, y, colour, count });
                    steps.push(Step::Sleep(1));
                    x += 1;
                    count += increment;
                }
                steps.push(Step::BarEnd { y, value });
                steps.push(Step::Sleep(12));
            }
            steps.push(Step::Sleep(60));
        }
        self.input.queue.clear();
        EndStats { screen: s, steps, next: 0, wait: 0, score, pal_channel: 765 + offset, pal_direction: 1, pal_timer: 0, pal_backup, ticks: 0 }
    }

    /// Without FAME.CPS (shareware): its layout, drawn in the interface's sand. Grooves of the
    /// same colours frame the two plaques at the top and the three stats boxes.
    fn end_stats_fallback_background(&mut self) -> Vec<u8> {
        let saved = std::mem::replace(&mut self.screen.pixels, vec![0; SCREEN_WIDTH * SCREEN_HEIGHT]);
        self.draw_sand_window(0, 0, SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32);
        let groove = |s: &mut Screen, x: i32, y: i32, w: i32, h: i32| {
            for (i, c) in [113u8, 105, 226, 227].into_iter().enumerate() {
                let i = i as i32;
                s.wired_rect(x + i, y + i, x + w - 1 - i, y + h - 1 - i, c);
            }
        };
        let s = &mut self.screen;
        groove(s, 64, 8, 192, 23);
        groove(s, 64, 32, 192, 31);
        groove(s, 0, 72, 320, 128);
        for i in 0..3 {
            let top = 80 + i * 36;
            s.fill_rect(8, top, 311, top + 34, 116);
            s.wired_rect(8, top, 311, top + 34, 228);
            s.fill_rect(270, top + 12, 304, top + 27, 226);
        }
        std::mem::replace(&mut self.screen.pixels, saved)
    }

    /// One tick of GUI_EndStats_Show's animation, then its wait for a key or click.
    /// True once it is over.
    fn end_stats_tick(&mut self, st: &mut EndStats) -> bool {
        st.ticks += 1;
        // GUI_HallOfFame_Tick
        if st.pal_timer < st.ticks {
            st.pal_timer = st.ticks + 2;
            let v = self.palette[st.pal_channel];
            if v >= 63 {
                st.pal_direction = -1;
            } else if v <= 35 {
                st.pal_direction = 1;
            }
            self.palette[st.pal_channel] = (v as i16 + st.pal_direction) as u8;
        }

        if st.next >= st.steps.len() {
            // Wait for any key or click.
            let key = std::iter::from_fn(|| self.input.queue.pop_front()).find(|&k| k != 0 && k & 0x800 == 0);
            return key.is_some();
        }
        self.input.queue.clear();
        if st.wait > 0 {
            st.wait -= 1;
            if st.wait > 0 {
                return false;
            }
        }
        let fonts = &self.fonts;
        while st.next < st.steps.len() {
            let step = st.steps[st.next];
            st.next += 1;
            let s = &mut st.screen;
            match step {
                Step::Sleep(n) => {
                    st.wait = n;
                    break;
                }
                Step::Rank => {
                    let t = self.game.string(rank(st.score));
                    s.draw_text_wrapper(fonts, &t, SCREEN_WIDTH as i32 / 2, 49, 6, 0, 0x122);
                }
                Step::Bar { x, y, colour, count } => {
                    s.fill_rect(271, y, 303, y + 5, 226);
                    s.draw_text_wrapper(fonts, count.to_string().as_bytes(), 287, y - 1, 0x14, 0, 0x121);
                    s.line(x, y, x, y + 5, colour);
                    s.line(x + 1, y + 1, x + 1, y + 6, 12);
                    self.sounds.push(SoundRequest::Effect(52));
                }
                Step::BarEnd { y, value } => {
                    s.fill_rect(271, y, 303, y + 5, 226);
                    s.draw_text_wrapper(fonts, value.to_string().as_bytes(), 287, y - 1, 0xF, 0, 0x121);
                    self.sounds.push(SoundRequest::Effect(38));
                }
            }
        }
        false
    }

    /// The end window's input; the button chosen, if any.
    fn end_window_input(&mut self, e: &mut EndScreen) -> Option<usize> {
        let mut choice = None;
        while let Some(k) = self.input.queue.pop_front() {
            let (x, y) = (self.input.click_x, self.input.click_y);
            match k {
                0x41 => e.pressed = button_at(x, y),
                0x841 => {
                    if let Some(b) = e.pressed.take() {
                        if button_at(x, y) == Some(b) {
                            choice = Some(b);
                        }
                    }
                }
                // Escape: carry on, as Escape closes the Options window.
                0x6E => choice = Some(CONTINUE),
                _ => {}
            }
        }
        choice
    }

    /// Whether the stats screen replaces the game on screen.
    pub(super) fn end_stats_shown(&self) -> bool {
        self.end.as_ref().is_some_and(|e| e.stage == EndStage::Stats && e.stats.is_some())
    }

    /// Draw the stats screen, or the end window over the (already drawn) game, with the arrow
    /// cursor as in the original's windows. False when there is nothing to draw.
    pub(super) fn end_render(&mut self) -> bool {
        let Some(e) = self.end.take() else { return false };
        let cursor = match e.stage {
            EndStage::Stats => {
                let st = e.stats.as_ref().unwrap();
                self.screen.pixels.copy_from_slice(&st.screen.pixels);
                // The mouse is hidden until the bars are done.
                st.next >= st.steps.len()
            }
            EndStage::Window => {
                self.shade_screen();
                self.draw_sand_window(WIN_X, WIN_Y, WIN_W, WIN_H);
                let ink = window_ink();
                let title: &[u8] = if e.won { b"Skirmish won" } else { b"Skirmish lost" };
                self.engraved_text(title, WIN_X + WIN_W / 2, WIN_Y + 7, ink, 1);
                let hover = button_at(self.input.mouse_x, self.input.mouse_y);
                let labels = [self.game.string(STR_RESTART_SCENARIO), b"New game".to_vec(), self.game.string(STR_CONTINUE_GAME)];
                for (i, label) in labels.iter().enumerate() {
                    let (x, y) = (WIN_X + BUTTON_X, WIN_Y + BUTTON_Y[i]);
                    let down = self.input.left_down && e.pressed == Some(i) && hover == Some(i);
                    self.gold_button(x, y, BUTTON_W, BUTTON_H, down);
                    let d = down as i32;
                    self.engraved_text(label, x + BUTTON_W / 2 + d, y + 2 + d, ink, 1);
                }
                true
            }
            _ => {
                self.end = Some(e);
                return false;
            }
        };
        self.end = Some(e);
        if cursor {
            let s = self.sprite(0).cloned();
            self.screen.draw_sprite(s.as_ref(), self.input.mouse_x, self.input.mouse_y, 0, 0, None, None);
        }
        true
    }
}
