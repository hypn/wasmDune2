//! The Options button's window: the original's "Game controls" window (GUI_Widget_GameControls_Click,
//! g_gameControlWindowDesc), opened directly. The rest of the original options window (load, save,
//! restart, pick another house) has no place in a skirmish. "Previous" becomes "Continue game"
//! (or Escape), which closes the window and resumes the game.
//!
//! Drawn like the setup screen (see setup.rs) over the shaded game, in the original layout.

use super::setup::window_ink;
use super::GameUi;
use crate::tables::strings::*;

// The window: the original's height and vertical layout (widget 16, g_gameControlWindowDesc),
// narrower than its 240 pixels (that width was for longer translations), centred on the screen.
const WIN_X: i32 = 60;
const WIN_Y: i32 = 48;
const WIN_W: i32 = 200;
const WIN_H: i32 = 134;
const BUTTON_H: i32 = 15;
const BUTTON_X: i32 = 100;

/// The buttons: (label string, offset x, offset y, width).
const BUTTONS: [(u16, i32, i32, i32); 6] = [
    (STR_MUSIC_IS, BUTTON_X, 22, 80),
    (STR_SOUNDS_ARE, BUTTON_X, 39, 80),
    (STR_GAME_SPEED, BUTTON_X, 56, 80),
    (STR_HINTS_ARE, BUTTON_X, 73, 80),
    (STR_AUTO_SCROLL_IS, BUTTON_X, 90, 80),
    (STR_NULL, (WIN_W - 136) / 2, 110, 136),
];
const CONTINUE: usize = 5;

#[derive(Default)]
pub struct OptionsWindow {
    /// The button held down with the left (false) or right (true) mouse button.
    pressed: Option<(usize, bool)>,
}

fn button_at(x: i32, y: i32) -> Option<usize> {
    let (x, y) = (x - WIN_X, y - WIN_Y);
    BUTTONS.iter().position(|&(_, bx, by, bw)| x >= bx && x < bx + bw && y >= by && y < by + BUTTON_H)
}

impl GameUi {
    /// Open the window (GUI_Widget_Options_Click): the game stops, and so does any voice.
    pub fn options_open(&mut self) {
        self.options = Some(OptionsWindow::default());
        if let Some(a) = &mut self.audio {
            a.stop_voice();
        }
    }

    /// One tick with the window open: only the window gets input.
    pub(super) fn options_tick(&mut self) {
        let Some(mut o) = self.options.take() else { return };
        let mut close = false;
        while let Some(k) = self.input.queue.pop_front() {
            let (x, y) = (self.input.click_x, self.input.click_y);
            match k {
                0x41 | 0x42 => o.pressed = button_at(x, y).map(|b| (b, k == 0x42)),
                0x841 | 0x842 => {
                    if let Some((b, back)) = o.pressed.take() {
                        if back == (k == 0x842) && button_at(x, y) == Some(b) {
                            close |= self.options_activate(b, back);
                        }
                    }
                }
                // Escape (the original "Previous" shortcut) closes the window.
                0x6E => close = true,
                _ => {}
            }
        }
        if !close {
            self.options = Some(o);
        }
    }

    /// A click on button `b` (right click = `back`); true when the window should close.
    fn options_activate(&mut self, b: usize, back: bool) -> bool {
        match b {
            0 => {
                if let Some(a) = &mut self.audio {
                    let on = !a.music_enabled;
                    a.set_music_enabled(on);
                }
            }
            1 => {
                if let Some(a) = &mut self.audio {
                    a.sounds_enabled = !a.sounds_enabled;
                    if !a.sounds_enabled {
                        a.stop_voice();
                    }
                }
            }
            2 => {
                let speed = &mut self.game.config.game_speed;
                *speed = if back { (*speed + 4) % 5 } else { (*speed + 1) % 5 };
            }
            3 => self.qol.no_hints = !self.qol.no_hints,
            4 => self.auto_scroll = !self.auto_scroll,
            _ => return true,
        }
        false
    }

    /// The window over the shaded (frozen) game.
    pub(super) fn options_render(&mut self) {
        let Some(o) = self.options.take() else { return };
        self.shade_screen();
        self.draw_sand_window(WIN_X, WIN_Y, WIN_W, WIN_H);
        let ink = window_ink();
        let title = self.game.string(STR_GAME_CONTROLS);
        self.engraved_text(&title, WIN_X + WIN_W / 2, WIN_Y + 7, ink, 1);

        let on_off = |on: bool| if on { STR_ON } else { STR_OFF };
        let music = self.audio.as_ref().is_some_and(|a| a.music_enabled);
        let sounds = self.audio.as_ref().is_some_and(|a| a.sounds_enabled);
        let values = [
            on_off(music),
            on_off(sounds),
            STR_SLOWEST + self.game.config.game_speed.min(4),
            on_off(!self.qol.no_hints),
            on_off(self.auto_scroll),
            STR_CONTINUE_GAME,
        ];
        let hover = button_at(self.input.mouse_x, self.input.mouse_y);
        let held = self.input.left_down || self.input.right_down;
        for (i, &(label, bx, by, bw)) in BUTTONS.iter().enumerate() {
            let (x, y) = (WIN_X + bx, WIN_Y + by);
            if label != STR_NULL {
                let t = self.game.string(label);
                self.engraved_text(&t, x - 8, y + 3, ink, 2);
            }
            let down = held && o.pressed.map(|p| p.0) == Some(i) && hover == Some(i);
            self.gold_button(x, y, bw, BUTTON_H, down);
            let d = down as i32;
            let t = self.game.string(values[i]);
            if i == CONTINUE {
                self.engraved_text(&t, x + bw / 2 + d, y + 2 + d, ink, 1);
            } else {
                self.engraved_text(&t, x + 4 + d, y + 2 + d, ink, 0);
            }
        }
        self.options = Some(o);
    }
}
