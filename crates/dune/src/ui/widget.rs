//! The original widget system (gui/widget.c): button state machine and hit testing.

use super::screen::WIDGET_PROPERTIES;

/// What a widget does when clicked / how it is drawn (replaces the C function pointers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetKind {
    /// Draws a sprite (normal/down), e.g. the Mentat/Options buttons.
    Sprite { normal: u16, down: u16 },
    SpriteButton,
    SpriteTextButton,
    TextButton2,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickProc {
    None,
    Mentat,
    Options,
    Name,
    Picture,
    RepairUpgrade,
    SpriteTextButton,
    Cancel,
    TextButton,
    Viewport,
}

#[derive(Clone, Debug)]
pub struct Widget {
    pub index: u16,
    pub shortcut: u16,
    pub shortcut2: u16,
    pub kind: WidgetKind,
    pub click: ClickProc,
    pub string_id: u16,
    // flags
    pub requires_click: bool,
    pub click_as_hover: bool,
    pub invisible: bool,
    pub grey_when_invisible: bool,
    pub no_click_cascade: bool,
    pub lose_select: bool,
    pub button_filter_left: u16,
    pub button_filter_right: u16,
    pub parent_id: usize,
    pub offset_x: i16,
    pub offset_y: i16,
    pub width: u16,
    pub height: u16,
    // state
    pub selected: bool,
    pub hover1: bool,
    pub hover2: bool,
    pub selected_last: bool,
    pub hover1_last: bool,
    pub key_selected: bool,
    pub button_state: u16,
}

impl Widget {
    /// Absolute screen position of the widget.
    pub fn position(&self) -> (i32, i32) {
        let wp = WIDGET_PROPERTIES[self.parent_id];
        let mut x = self.offset_x as i32;
        if self.offset_x < 0 {
            x += (wp.2 * 8) as i32;
        }
        x += (wp.0 * 8) as i32;
        let mut y = self.offset_y as i32;
        if self.offset_y < 0 {
            y += wp.3 as i32;
        }
        y += wp.1 as i32;
        (x, y)
    }
}

/// Input state shared with the widget event handler (g_mouseX, the key buffer, ...).
#[derive(Default)]
pub struct Input {
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub click_x: i32,
    pub click_y: i32,
    /// Pending key/mouse events in the original encoding: 0x41 = left button, 0x42 = right
    /// button, | 0x0800 = release; other values are keyboard scancodes.
    pub queue: std::collections::VecDeque<u16>,
    /// Pressed state of keys (for Input_Test), indexed by scancode.
    pub held: Vec<bool>,
    pub left_down: bool,
    pub right_down: bool,
}

impl Input {
    /// A copy of the pointer/keyboard state without the event queue.
    pub fn snapshot(&self) -> Input {
        Input {
            mouse_x: self.mouse_x,
            mouse_y: self.mouse_y,
            click_x: self.click_x,
            click_y: self.click_y,
            queue: Default::default(),
            held: self.held.clone(),
            left_down: self.left_down,
            right_down: self.right_down,
        }
    }

    pub fn push_mouse(&mut self, right: bool, down: bool, x: i32, y: i32) {
        self.click_x = x;
        self.click_y = y;
        if right {
            self.right_down = down;
        } else {
            self.left_down = down;
        }
        let code = if right { 0x42 } else { 0x41 } | if down { 0 } else { 0x0800 };
        self.queue.push_back(code);
    }

    pub fn push_key(&mut self, scancode: u8, down: bool) {
        if self.held.len() < 256 {
            self.held.resize(256, false);
        }
        self.held[scancode as usize] = down;
        if down {
            self.queue.push_back(scancode as u16);
        }
    }

    pub fn test(&self, code: u16) -> bool {
        match code {
            0x41 => self.left_down,
            0x42 => self.right_down,
            c => self.held.get(c as usize).copied().unwrap_or(false),
        }
    }
}

/// The widget list with the static state of GUI_Widget_HandleEvents.
#[derive(Default)]
pub struct Widgets {
    pub list: Vec<Widget>,
    selected: Option<usize>,
    button_state: u16,
    pub reset: bool,
}

impl Widgets {
    pub fn get(&self, index: u16) -> Option<&Widget> {
        self.list.iter().find(|w| w.index == index)
    }

    pub fn get_mut(&mut self, index: u16) -> Option<&mut Widget> {
        self.list.iter_mut().find(|w| w.index == index)
    }

    /// GUI_Widget_Insert (sorted by index).
    pub fn insert(&mut self, w: Widget) {
        let pos = self.list.iter().position(|x| x.index >= w.index).unwrap_or(self.list.len());
        self.list.insert(pos, w);
        self.reset = true;
    }

    pub fn make_visible(&mut self, index: u16, visible: bool) {
        if let Some(w) = self.get_mut(index) {
            w.invisible = !visible;
        }
    }

    pub fn make_selected(&mut self, index: u16, selected: bool) {
        if let Some(w) = self.get_mut(index) {
            if w.invisible {
                return;
            }
            w.selected_last = w.selected;
            w.selected = selected;
            if !selected {
                w.hover1 = false;
                w.hover2 = false;
            }
        }
    }

    /// GUI_Widget_HandleEvents. `on_click(widgets, index)` runs a widget's click proc and
    /// returns true to stop handling further widgets. Returns the unhandled key.
    pub fn handle_events(&mut self, input: &mut Input, on_click: &mut dyn FnMut(&mut Widgets, usize) -> bool) -> u16 {
        let mut key: u16 = input.queue.pop_front().unwrap_or(0);

        if self.reset {
            self.reset = false;
            self.selected = None;
            self.button_state = 0;
            if input.test(0x41) {
                self.button_state |= 0x0200;
            }
            if input.test(0x42) {
                self.button_state |= 0x2000;
            }
        }

        let mut mouse_x = input.mouse_x;
        let mut mouse_y = input.mouse_y;
        let mut button_state: u16 = 0;
        {
            let mut change: u16 = 0;
            if (key & 0x00FF) == 0x42 {
                change = 0x1000;
            }
            if (key & 0x00FF) == 0x41 {
                change = 0x0100;
            }
            if key & 0x0800 != 0 {
                change <<= 2;
            }
            if change != 0 {
                mouse_x = input.click_x;
                mouse_y = input.click_y;
            }
            self.button_state &= !((change & 0x4400) >> 1);
            self.button_state |= (change & 0x1100) << 1;
            button_state |= change;
            button_state |= self.button_state;
            button_state |= (self.button_state << 2) ^ 0x8800;
        }

        let mut start = 0usize;
        if let Some(sel) = self.selected {
            start = sel;
            if self.list[sel].invisible {
                self.selected = None;
            }
        }

        let mut return_value: u16 = 0;
        if std::env::var("UIDBG2").is_ok() && key != 0 {
            eprintln!("events key={:x} bs={:x} start={} sel={:?} mouse=({},{}) visible={:?}", key, button_state, start, self.selected, mouse_x, mouse_y,
                self.list.iter().filter(|w| !w.invisible).map(|w| w.index).collect::<Vec<_>>());
        }
        let mut i = start;
        while i < self.list.len() {
            let w = &mut self.list[i];
            if w.invisible {
                i += 1;
                continue;
            }
            w.selected_last = w.selected;
            w.hover1_last = w.hover1;
            let (px, py) = w.position();
            let mut hover = px <= mouse_x && mouse_x <= px + w.width as i32 && py <= mouse_y && mouse_y <= py + w.height as i32;
            w.key_selected = false;
            if (key & 0x7F) != 0 && ((key & 0x7F) == w.shortcut || (key & 0x7F) == w.shortcut2) {
                hover = true;
                w.key_selected = true;
                key = 0;
                button_state = w.button_filter_left << 8;
                self.selected = Some(i);
            }
            w.hover1 = false;
            w.hover2 = false;
            if hover {
                if button_state & 0x3300 != 0 && w.click_as_hover && (self.selected == Some(i) || self.selected.is_none()) {
                    w.hover1 = true;
                    w.hover2 = true;
                    if self.selected.is_none() {
                        self.selected = Some(i);
                    }
                }
                if button_state & 0x8800 != 0 && !w.click_as_hover {
                    w.hover1 = true;
                    w.hover2 = true;
                }
            }
            let mut trigger = hover;
            if let Some(sel) = self.selected {
                if self.list[sel].lose_select {
                    trigger = sel == i;
                }
            }
            let w = &mut self.list[i];
            let mut widget_click = false;
            if trigger {
                if button_state & 0x1100 != 0 && self.selected.is_none() {
                    self.selected = Some(i);
                    key = 0;
                }
                let left = (button_state >> 8) & w.button_filter_left;
                let right = (button_state >> 12) & w.button_filter_right;
                if (left != 0 || right != 0) && (hover || !w.requires_click) {
                    if left & 1 != 0 || right & 1 != 0 {
                        w.selected = !w.selected;
                        return_value = w.index | 0x8000;
                        widget_click = true;
                        if w.click_as_hover {
                            w.hover1 = true;
                            w.hover2 = true;
                        }
                        self.selected = Some(i);
                    } else if left & 2 != 0 || right & 2 != 0 {
                        if !w.click_as_hover {
                            w.hover1 = true;
                            w.hover2 = true;
                        }
                        if !w.requires_click {
                            widget_click = true;
                        }
                    } else if left & 4 != 0 || right & 4 != 0 {
                        if !w.requires_click || self.selected == Some(i) {
                            w.selected = !w.selected;
                            return_value = w.index | 0x8000;
                            widget_click = true;
                        }
                        if !w.click_as_hover {
                            w.hover1 = false;
                            w.hover2 = false;
                        }
                    } else {
                        if w.click_as_hover {
                            w.hover1 = true;
                            w.hover2 = true;
                        }
                        if !w.requires_click {
                            widget_click = true;
                        }
                    }
                }
            }
            let mut fake_click = false;
            if hover && button_state & 0x2200 != 0 {
                w.hover1 = true;
                w.hover2 = true;
                if !w.click_as_hover && !w.selected {
                    fake_click = true;
                    w.selected = true;
                }
            }
            if button_state & 0x8800 == 0x8800 {
                self.selected = None;
                if !hover || w.click_as_hover {
                    w.hover1 = false;
                    w.hover2 = false;
                }
            }
            if !hover && self.selected == Some(i) && !w.lose_select {
                self.selected = None;
            }
            if fake_click {
                w.selected = false;
            }
            if widget_click {
                w.button_state = button_state >> 8;
                let no_cascade = w.no_click_cascade;
                if on_click(self, i) {
                    break;
                }
                if no_cascade {
                    break;
                }
            }
            if self.selected == Some(i) && self.list[i].lose_select {
                break;
            }
            i += 1;
        }
        let _ = return_value;
        key & 0x7FFF
    }
}

/// GUI_Widget_GetShortcut: ASCII -> scancode (US layout), as used for widget shortcuts.
pub fn get_shortcut(c: u8) -> u16 {
    const SHORTCUTS: [u8; 128] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0x0f, 0x10, 0, 0, 0, 0x2b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x6e, 0, 0, 0, 0, 0x3d, 0x02, 0x29,
        0x04, 0x05, 0x06, 0x08, 0x29, 0x0a, 0x0b, 0x64, 0x6a, 0x35, 0x0c, 0x36, 0x5f, 0x0b, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0a, 0x28, 0x28, 0x35, 0x0d, 0x36, 0x41, 0x03, 0x1f, 0x32, 0x30, 0x21, 0x13, 0x22, 0x23, 0x24, 0x18, 0x25, 0x26, 0x27,
        0x34, 0x33, 0x19, 0x1a, 0x11, 0x14, 0x20, 0x15, 0x17, 0x31, 0x12, 0x2f, 0x16, 0x2e, 0x1b, 0x1d, 0x1c, 0x07, 0x0c, 0x01, 0x1f,
        0x32, 0x30, 0x21, 0x13, 0x22, 0x23, 0x24, 0x18, 0x25, 0x26, 0x27, 0x34, 0x33, 0x19, 0x1a, 0x11, 0x14, 0x20, 0x15, 0x17, 0x31,
        0x12, 0x2f, 0x16, 0x2e, 0x1b, 0x1d, 0x1c, 0x01, 0x00,
    ];
    SHORTCUTS.get(c as usize).copied().unwrap_or(0) as u16
}

/// Map a browser KeyboardEvent.code to the scancodes the original uses.
pub fn scancode_from_code(code: &str) -> u8 {
    let c = match code {
        "Escape" => 0x6e,
        "Backquote" => 0x01,
        "Digit1" => 0x02,
        "Digit2" => 0x03,
        "Digit3" => 0x04,
        "Digit4" => 0x05,
        "Digit5" => 0x06,
        "Digit6" => 0x07,
        "Digit7" => 0x08,
        "Digit8" => 0x09,
        "Digit9" => 0x0a,
        "Digit0" => 0x0b,
        "Minus" => 0x0c,
        "Equal" => 0x0d,
        "Backspace" => 0x0f,
        "Tab" => 0x10,
        "KeyQ" => 0x11,
        "KeyW" => 0x12,
        "KeyE" => 0x13,
        "KeyR" => 0x14,
        "KeyT" => 0x15,
        "KeyY" => 0x16,
        "KeyU" => 0x17,
        "KeyI" => 0x18,
        "KeyO" => 0x19,
        "KeyP" => 0x1a,
        "BracketLeft" => 0x1b,
        "BracketRight" => 0x1c,
        "Backslash" => 0x1d,
        "KeyA" => 0x1f,
        "KeyS" => 0x20,
        "KeyD" => 0x21,
        "KeyF" => 0x22,
        "KeyG" => 0x23,
        "KeyH" => 0x24,
        "KeyJ" => 0x25,
        "KeyK" => 0x26,
        "KeyL" => 0x27,
        "Semicolon" => 0x28,
        "Quote" => 0x29,
        "Enter" => 0x2b,
        "ShiftLeft" => 0x2c,
        "KeyZ" => 0x2e,
        "KeyX" => 0x2f,
        "KeyC" => 0x30,
        "KeyV" => 0x31,
        "KeyB" => 0x32,
        "KeyN" => 0x33,
        "KeyM" => 0x34,
        "Comma" => 0x35,
        "Period" => 0x36,
        "Slash" => 0x37,
        "ShiftRight" => 0x39,
        "Space" => 0x3d,
        "Insert" => 0x4b,
        "Delete" => 0x4c,
        "ArrowLeft" => 0x4f,
        "Home" => 0x50,
        "End" => 0x51,
        "ArrowUp" => 0x53,
        "ArrowDown" => 0x54,
        "PageUp" => 0x55,
        "PageDown" => 0x56,
        "ArrowRight" => 0x59,
        "Numpad7" => 0x5b,
        "Numpad4" => 0x5c,
        "Numpad1" => 0x5d,
        "Numpad8" => 0x60,
        "Numpad5" => 0x61,
        "Numpad2" => 0x62,
        "Numpad0" => 0x63,
        "NumpadMultiply" => 0x64,
        "Numpad9" => 0x65,
        "Numpad6" => 0x66,
        "Numpad3" => 0x67,
        "NumpadDecimal" => 0x68,
        "NumpadSubtract" => 0x69,
        "NumpadAdd" => 0x6a,
        "F1" => 0x70,
        "F2" => 0x71,
        "F3" => 0x72,
        "F4" => 0x73,
        "F5" => 0x74,
        "F6" => 0x75,
        "F7" => 0x76,
        "F8" => 0x77,
        "F9" => 0x78,
        "F10" => 0x79,
        _ => 0,
    };
    c
}
