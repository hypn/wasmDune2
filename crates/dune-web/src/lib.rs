//! WebAssembly front-end: a small C-style ABI driven by web/main.js (no wasm-bindgen).

use dune::data::{pak::Vfs, Assets, DataVariant};
use dune::game::skirmish::{AiDifficulty, SkirmishSettings, StartForce};
use dune::game::Game;
use dune::ui::screen::{SCREEN_HEIGHT, SCREEN_WIDTH};
use dune::ui::widget::scancode_from_code;
use dune::ui::{GameUi, SoundRequest};
use std::cell::RefCell;

thread_local! {
    static VFS: RefCell<Option<Vfs>> = const { RefCell::new(None) };
    static UI: RefCell<Option<GameUi>> = const { RefCell::new(None) };
    static ERROR: RefCell<String> = const { RefCell::new(String::new()) };
    static RGBA: RefCell<Vec<u8>> = RefCell::new(vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * 4]);
    static SOUNDS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

fn set_error(e: String) -> i32 {
    ERROR.with(|err| *err.borrow_mut() = e);
    -1
}

unsafe fn slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, len)
    }
}

fn with_ui<R>(f: impl FnOnce(&mut GameUi) -> R) -> Option<R> {
    UI.with(|u| u.borrow_mut().as_mut().map(f))
}

#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr`/`len` must come from `alloc`.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len));
}

#[no_mangle]
pub extern "C" fn error_ptr() -> *const u8 {
    ERROR.with(|e| e.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn error_len() -> usize {
    ERROR.with(|e| e.borrow().len())
}

/// # Safety
/// Pointers must be valid for the given lengths.
#[no_mangle]
pub unsafe extern "C" fn add_pak(name: *const u8, name_len: usize, data: *const u8, data_len: usize) -> i32 {
    let name = String::from_utf8_lossy(slice(name, name_len)).to_string();
    let data = slice(data, data_len);
    VFS.with(|v| {
        let mut v = v.borrow_mut();
        let vfs = v.get_or_insert_with(Vfs::new);
        match vfs.add_pak(&name, data) {
            Ok(()) => 0,
            Err(e) => set_error(format!("{name}: {e}")),
        }
    })
}

/// Decode all assets. Returns 0 = full game, 1 = shareware, -1 = error.
#[no_mangle]
pub extern "C" fn init() -> i32 {
    let Some(vfs) = VFS.with(|v| v.borrow_mut().take()) else {
        return set_error("no PAK files loaded".into());
    };
    let assets = match Assets::load(vfs) {
        Ok(a) => a,
        Err(e) => return set_error(e),
    };
    let variant = assets.variant;
    let game = match Game::new(&assets) {
        Ok(g) => g,
        Err(e) => return set_error(e),
    };
    UI.with(|u| *u.borrow_mut() = Some(GameUi::new(assets, game)));
    match variant {
        DataVariant::Full => 0,
        DataVariant::Shareware => 1,
    }
}

/// Start a skirmish. `opponents` is a bitmask of houses (bit = house id). Returns the map seed used.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn start_skirmish(
    player_house: u32,
    opponents: u32,
    seed: u32,
    map_scale: u32,
    credits: u32,
    tech_level: u32,
    difficulty: u32,
    start_force: u32,
    ai_allied: u32,
    game_speed: u32,
    enhanced: u32,
) -> i32 {
    let settings = SkirmishSettings {
        player_house: player_house as u8,
        opponents: (0..6u8).filter(|h| opponents & (1 << h) != 0 && *h as u32 != player_house).collect(),
        map_seed: seed,
        map_scale: map_scale as u16,
        credits: credits as u16,
        tech_level: tech_level as u16,
        difficulty: match difficulty {
            0 => AiDifficulty::Easy,
            2 => AiDifficulty::Hard,
            _ => AiDifficulty::Normal,
        },
        start_force: match start_force {
            0 => StartForce::McvOnly,
            2 => StartForce::Medium,
            _ => StartForce::Small,
        },
        ai_allied: ai_allied != 0,
        game_speed: game_speed as u16,
        dune2_enhanced: enhanced != 0,
    };
    with_ui(|ui| {
        let GameUi { game, assets, .. } = ui;
        game.start_skirmish(assets, &settings);
        ui.start();
        ui.game.scenario.map_seed as i32
    })
    .unwrap_or(-1)
}

/// Advance `n` 60 Hz ticks.
#[no_mangle]
pub extern "C" fn tick(n: u32) {
    with_ui(|ui| {
        for _ in 0..n.min(30) {
            ui.tick();
        }
        let sounds: Vec<u32> = ui
            .sounds
            .drain(..)
            .map(|s| match s {
                SoundRequest::Feedback(id) => 0x10000 | id as u32,
                SoundRequest::Voice(id) => id as u32,
            })
            .collect();
        SOUNDS.with(|s| s.borrow_mut().extend(sounds));
    });
}

/// Render and return a pointer to the 320x200 RGBA framebuffer.
#[no_mangle]
pub extern "C" fn render() -> *const u8 {
    with_ui(|ui| {
        ui.render();
        RGBA.with(|rgba| {
            let mut rgba = rgba.borrow_mut();
            ui.to_rgba(&mut rgba);
            rgba.as_ptr()
        })
    })
    .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn mouse_move(x: i32, y: i32) {
    with_ui(|ui| {
        ui.input.mouse_x = x.clamp(0, SCREEN_WIDTH as i32 - 1);
        ui.input.mouse_y = y.clamp(0, SCREEN_HEIGHT as i32 - 1);
    });
}

/// button: 0 = left, 2 = right.
#[no_mangle]
pub extern "C" fn mouse_button(button: u32, down: u32, x: i32, y: i32) {
    with_ui(|ui| {
        ui.input.mouse_x = x.clamp(0, SCREEN_WIDTH as i32 - 1);
        ui.input.mouse_y = y.clamp(0, SCREEN_HEIGHT as i32 - 1);
        ui.input.push_mouse(button == 2, down != 0, ui.input.mouse_x, ui.input.mouse_y);
    });
}

/// # Safety
/// `code` must be valid for `len` bytes (a KeyboardEvent.code string).
#[no_mangle]
pub unsafe extern "C" fn key(code: *const u8, len: usize, down: u32) {
    let code = String::from_utf8_lossy(slice(code, len)).to_string();
    let sc = scancode_from_code(&code);
    if sc == 0 {
        return;
    }
    with_ui(|ui| ui.input.push_key(sc, down != 0));
}

/// Pop the next sound request (0 = none; 0x10000 | id = feedback, else voice id + 1).
#[no_mangle]
pub extern "C" fn next_sound() -> u32 {
    SOUNDS.with(|s| {
        let mut s = s.borrow_mut();
        if s.is_empty() {
            0
        } else {
            let v = s.remove(0);
            if v & 0x10000 != 0 {
                v
            } else {
                v + 1
            }
        }
    })
}

/// 0 = playing, 1 = won, 2 = lost.
#[no_mangle]
pub extern "C" fn game_state() -> u32 {
    with_ui(|ui| match ui.game.skirmish.finished {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    })
    .unwrap_or(0)
}

/// Quality-of-life toggles: bit 0 = right click cancels, bit 1 = no hints, bit 2 = right click orders.
#[no_mangle]
pub extern "C" fn set_qol(flags: u32) {
    with_ui(|ui| {
        ui.qol.right_click_cancel = flags & 1 != 0;
        ui.qol.right_click_orders = flags & 4 != 0;
        ui.qol.no_hints = flags & 2 != 0;
    });
}

/// Debug options, applied after `start_skirmish`: bit 0 = no fog of war.
#[no_mangle]
pub extern "C" fn set_debug(flags: u32) {
    with_ui(|ui| {
        if flags & 1 != 0 {
            ui.game.debug_reveal_map();
            ui.game.events.clear();
        }
    });
}

/// Turbo: simulation speed in percent of the original pace (100 = original).
#[no_mangle]
pub extern "C" fn set_sim_speed(percent: u32) {
    with_ui(|ui| ui.sim_percent = percent.clamp(25, 800));
}
