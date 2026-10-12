//! Developer tool: inspect the original data files and dump images as PNG.
//!
//! usage: dune-tool <datadir> list
//!        dune-tool <datadir> map <seed> <out.png>
//!        dune-tool <datadir> cps <NAME.CPS> <out.png>
//!        dune-tool <datadir> tiles <out.png>
//!        dune-tool <datadir> sprites <out.png>

use dune::data::image::{load_cps, Palette};
use dune::data::{pak::Vfs, Assets, KNOWN_PAKS};
use dune::gfx::Surface;
use std::path::Path;

fn load_vfs(dir: &Path) -> Vfs {
    let mut vfs = Vfs::new();
    for name in KNOWN_PAKS {
        let path = dir.join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            vfs.add_pak(name, &bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }
    vfs
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Minimal PNG writer (RGBA8, stored deflate blocks).
fn write_png(path: &str, w: usize, h: usize, rgba: &[u8]) {
    let mut raw = Vec::with_capacity((w * 4 + 1) * h);
    for y in 0..h {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * w * 4..(y + 1) * w * 4]);
    }
    let mut z = vec![0x78, 0x01];
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, c) in chunks.iter().enumerate() {
        z.push((i == chunks.len() - 1) as u8);
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |ty: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = ty.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    std::fs::write(path, out).unwrap();
}

fn save(surface: &Surface, pal: &Palette, path: &str) {
    let mut rgba = vec![0u8; surface.width * surface.height * 4];
    surface.to_rgba(&pal.to_rgba(), &mut rgba);
    write_png(path, surface.width, surface.height, &rgba);
    println!("wrote {path} ({}x{})", surface.width, surface.height);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: dune-tool <datadir> <list|map|cps|tiles|sprites> ...");
        std::process::exit(1);
    }
    let vfs = load_vfs(Path::new(&args[1]));
    match args[2].as_str() {
        "list" => {
            let mut names: Vec<_> = vfs.names().cloned().collect();
            names.sort();
            println!("{}", names.join(" "));
            let assets = Assets::load(vfs).unwrap();
            println!(
                "variant: {:?}, tiles: {} ({}x{}), sprites: {} ({} present)",
                assets.variant,
                assets.tiles.count(),
                assets.tiles.width,
                assets.tiles.height,
                assets.sprites.len(),
                assets.sprites.iter().filter(|s| s.is_some()).count()
            );
        }
        "map" => {
            let seed: u32 = args[3].parse().unwrap();
            let assets = Assets::load(vfs).unwrap();
            let mut map = dune::map::Map::default();
            let mut rng = dune::rng::Rng::new();
            dune::map::landscape::create_landscape(&mut map, &mut rng, &assets.tiles, seed);
            let mut s = Surface::new(64 * 16, 64 * 16);
            for (i, t) in map.tiles.iter().enumerate() {
                s.draw_tile(&assets.tiles, t.ground_tile_id, (i % 64) as i32 * 16, (i / 64) as i32 * 16, 0);
            }
            save(&s, &assets.palette, &args[4]);
        }
        "cps" => {
            let cps = load_cps(vfs.get(&args[3]).expect("no such file"));
            let pal = cps.palette.clone().unwrap_or_else(|| Palette::from_bytes(vfs.get("IBM.PAL").unwrap()));
            let mut s = Surface::new(320, 200);
            s.pixels.copy_from_slice(&cps.pixels);
            save(&s, &pal, &args[4]);
        }
        "tiles" => {
            let assets = Assets::load(vfs).unwrap();
            let n = assets.tiles.count();
            let cols = 32;
            let mut s = Surface::new(cols * 17, n.div_ceil(cols) * 17);
            for i in 0..n {
                s.draw_tile(&assets.tiles, i as u16, (i % cols) as i32 * 17, (i / cols) as i32 * 17, 0);
            }
            save(&s, &assets.palette, &args[3]);
        }
        "sprites" => {
            let assets = Assets::load(vfs).unwrap();
            let cell = 48;
            let cols = 24;
            let n = assets.sprites.len();
            let mut s = Surface::new(cols * cell, n.div_ceil(cols) * cell);
            for (i, sp) in assets.sprites.iter().enumerate() {
                let Some(sp) = sp else { continue };
                let px = sp.expand();
                let (ox, oy) = ((i % cols) * cell, (i / cols) * cell);
                for y in 0..(sp.height as usize).min(cell) {
                    for x in 0..(sp.width as usize).min(cell) {
                        let v = px[y * sp.width as usize + x];
                        if v != 0 {
                            s.pixels[(oy + y) * s.width + ox + x] = v;
                        }
                    }
                }
            }
            save(&s, &assets.palette, &args[3]);
        }
        "sim" => {
            use dune::game::{skirmish::SkirmishSettings, Game};
            let assets = Assets::load(vfs).unwrap();
            let mut game = Game::new(&assets).unwrap();
            let seed: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(27);
            let minutes: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(10);
            let speed: u16 = std::env::var("SPEED").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
            // ALLIED=0: free-for-all, so the computers also fight (and rebuild) among themselves.
            let ai_allied = std::env::var("ALLIED").as_deref() != Ok("0");
            let settings = SkirmishSettings { map_seed: seed, opponents: vec![0, 2], game_speed: speed, ai_allied, ..Default::default() };
            game.start_skirmish(&assets, &settings);
            // Let the (absent) human deploy their MCV too.
            for u in game.units_of(game.player_house_id, dune::game::UNIT_MCV as u16) {
                game.unit_set_action(u, dune::game::ACTION_DEPLOY);
            }
            for minute in 0..=minutes {
                if minute > 0 {
                    for _ in 0..std::env::var("SIMSTEP").ok().and_then(|v| v.parse().ok()).unwrap_or(3600) {
                        game.tick();
                    }
                }
                let mut line = format!("t={minute:>3}m");
                for h in game.house_ids() {
                    let units = game.units_of(h, 0xFFFF).len();
                    let structs: Vec<String> = game
                        .structures_of(h, 0xFFFF)
                        .iter()
                        .map(|&s| dune::tables::STRUCTURE_INFO[game.s(s).o.type_ as usize].o.name.chars().take(4).collect())
                        .collect();
                    let hh = game.h(h);
                    line += &format!(" | {} cr={} pw={}/{} u={} s=[{}]", dune::tables::HOUSE_INFO[h as usize].name, hh.credits, hh.power_production, hh.power_usage, units, structs.join(","));
                }
                println!("{line}");
                // Structures must never share a tile.
                let mut owner = vec![None; 4096];
                for st in game.structures_of(0xFF, 0xFFFF) {
                    let o = &game.s(st).o;
                    let layout = dune::tables::STRUCTURE_INFO[o.type_ as usize].layout as usize;
                    for i in 0..dune::tables::STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
                        let tile = (o.position.pack().wrapping_add(dune::tables::STRUCTURE_LAYOUT_TILES[layout][i]) & 0xFFF) as usize;
                        if let Some(other) = owner[tile] {
                            let name = |s: u16| dune::tables::STRUCTURE_INFO[game.s(s).o.type_ as usize].o.name;
                            println!("      OVERLAP at tile {tile:#x}: {} {st} and {} {other} (house {})", name(st), name(other), o.house_id);
                        }
                        owner[tile] = Some(st);
                    }
                }
                if std::env::var("SIMDBG").is_ok() {
                    for u in game.units_of(0xFF, 0xFFFF) {
                        let un = game.u(u);
                        println!("      unit {} {} house={} act={} pos=({},{}) amount={} tm={:04x} ta={:04x} hp={} v4={:04x} linked={}",
                            u, dune::tables::UNIT_INFO[un.o.type_ as usize].o.name, un.o.house_id, un.action_id,
                            un.o.position.x >> 8, un.o.position.y >> 8, un.amount, un.target_move, un.target_attack, un.o.hitpoints, un.o.script.variables[4], un.o.linked_id);
                    }
                    if let Ok(t) = std::env::var("SIMTILE") {
                        let v: Vec<u16> = t.split(',').map(|x| x.parse().unwrap()).collect();
                        let p = dune::tile::pack_xy(v[0], v[1]);
                        let mt = &game.map[p as usize];
                        println!("      tile ({},{}) lst={} ground={} overlay={} has_unit={} idx={} search_spice={:x} valid={}", v[0], v[1], game.map_get_landscape_type(p), mt.ground_tile_id, mt.overlay_tile_id, mt.has_unit, mt.index, game.map_search_spice(p, 20), game.map_is_valid_position(p));
                    }
                    game.validate_strict_if_zero += 1;
                    for u in game.units_of(0xFF, 0xFFFF) {
                        let un = game.u(u);
                        if un.o.flags.is_not_on_map { println!("      offmap unit {} {} house={} amount={} act={} script={:?}", u, dune::tables::UNIT_INFO[un.o.type_ as usize].o.name, un.o.house_id, un.amount, un.action_id, un.o.script.script); }
                    }
                    game.validate_strict_if_zero -= 1;
                    for s in game.structures_of(0xFF, 0xFFFF) {
                        let st = game.s(s);
                        println!("      struct {} {} house={} state={} linked={} cd={} v4={:04x} hp={} ip={:?} delay={}", s, dune::tables::STRUCTURE_INFO[st.o.type_ as usize].o.name, st.o.house_id, st.state, st.o.linked_id, st.count_down, st.o.script.variables[4], st.o.hitpoints, st.o.script.script, st.o.script.delay);
                    }
                }
                let texts: Vec<String> = game
                    .events
                    .drain(..)
                    .filter_map(|e| match e {
                        dune::game::Event::Text(t, _) => Some(String::from_utf8_lossy(&t).to_string()),
                        _ => None,
                    })
                    .collect();
                if !texts.is_empty() {
                    println!("    msgs: {}", texts.iter().take(6).cloned().collect::<Vec<_>>().join(" / "));
                }
                if let Some(w) = game.skirmish.finished {
                    println!("finished: player {}", if w { "won" } else { "lost" });
                    break;
                }
            }
        }
        "ui" => {
            // dune-tool <data> ui <seed> <ticks> <out.png> [click x,y[,r]@tick ...]
            use dune::game::{skirmish::SkirmishSettings, Game};
            use dune::ui::GameUi;
            let assets = Assets::load(vfs).unwrap();
            let game = Game::new(&assets).unwrap();
            let seed: u32 = args[3].parse().unwrap();
            let ticks: u32 = args[4].parse().unwrap();
            let mut ui = GameUi::new(assets, game);
            let settings = SkirmishSettings { map_seed: seed, opponents: vec![0], ..Default::default() };
            {
                let GameUi { game, assets, .. } = &mut ui;
                game.start_skirmish(assets, &settings);
            }
            ui.start();
            // DEPLOY=1 deploys the player's MCV straight away (as `sim` does).
            if std::env::var("DEPLOY").is_ok() {
                for u in ui.game.units_of(ui.game.player_house_id, dune::game::UNIT_MCV as u16) {
                    ui.game.unit_set_action(u, dune::game::ACTION_DEPLOY);
                }
            }
            if std::env::var("NOFOG").is_ok() {
                ui.game.debug_reveal_map();
                ui.debug_full_radar = true;
            }
            if let Ok(q) = std::env::var("QOL") {
                let q: u32 = q.parse().unwrap();
                ui.qol.right_click_cancel = q & 1 != 0;
                ui.qol.no_hints = q & 2 != 0;
                ui.qol.right_click_orders = q & 4 != 0;
                ui.qol.screen_edge_scroll = q & 8 != 0;
                ui.qol.home_key = q & 16 != 0;
            }
            // AUDIO=1: run the audio engine too (rendered and discarded) and report what played.
            let audio = std::env::var("AUDIO").is_ok();
            if audio {
                ui.audio_init(22050);
                let a = ui.audio.as_ref().unwrap();
                println!("voices loaded: {}", a.voices_loaded());
                if std::env::var("AUDIO").as_deref() == Ok("2") {
                    for (i, v) in dune::tables::sound::VOICES.iter().enumerate() {
                        println!("  {i:3} {:16} {}", v.0, if a.voice_loaded(i) { "ok" } else { "-" });
                    }
                }
            }
            // HOVER=x,y parks the mouse there (no buttons), e.g. to test edge scrolling.
            if let Ok(h) = std::env::var("HOVER") {
                let (x, y) = h.split_once(',').unwrap();
                ui.input.mouse_x = x.parse().unwrap();
                ui.input.mouse_y = y.parse().unwrap();
            }
            // SETUP=1 shows the skirmish setup screen instead (clicks go to it).
            if std::env::var("SETUP").is_ok() {
                let shareware = ui.assets.variant == dune::data::DataVariant::Shareware;
                ui.setup_open(shareware, seed);
            }
            let mut clicks: Vec<(u32, i32, i32, bool)> = Vec::new();
            // key:<KeyboardEvent.code>@tick presses (and releases) a key.
            let mut keys: Vec<(u32, u8)> = Vec::new();
            for a in &args[6..] {
                let (pos, t) = a.split_once('@').unwrap();
                if let Some(code) = pos.strip_prefix("key:") {
                    keys.push((t.parse().unwrap(), dune::ui::widget::scancode_from_code(code)));
                    continue;
                }
                let v: Vec<&str> = pos.split(',').collect();
                clicks.push((t.parse().unwrap(), v[0].parse().unwrap(), v[1].parse().unwrap(), v.get(2) == Some(&"r")));
            }
            for t in 0..ticks {
                for &(ct, x, y, r) in &clicks {
                    if ct == t {
                        ui.input.mouse_x = x;
                        ui.input.mouse_y = y;
                        ui.input.push_mouse(r, true, x, y);
                    }
                    if ct + 2 == t {
                        ui.input.push_mouse(r, false, x, y);
                    }
                }
                for &(kt, sc) in &keys {
                    if kt == t || kt + 2 == t {
                        ui.input.push_key(sc, kt == t);
                    }
                }
                ui.tick();
                if let Some(a) = &mut ui.audio {
                    a.skip(22050 / 60);
                }
            }
            // FACTORY=1 opens the build window of a computer's construction yard (render test).
            if std::env::var("FACTORY").is_ok() {
                use dune::game::STRUCTURE_CONSTRUCTION_YARD;
                let cy = (0..6u8).flat_map(|h| ui.game.structures_of(h, STRUCTURE_CONSTRUCTION_YARD as u16)).next();
                if let Some(cy) = cy {
                    let f = dune::ui::factory::FactoryWindow::new(&mut ui, cy, true, false, 0);
                    ui.factory = Some(f);
                }
            }
            if let Some(a) = &ui.audio {
                println!("audio: {} voices, {} AdLib effects, {} songs started", a.stats[0], a.stats[1], a.stats[2]);
            }
            ui.render();
            let mut rgba = vec![0u8; 320 * 200 * 4];
            ui.to_rgba(&mut rgba);
            write_png(&args[5], 320, 200, &rgba);
            println!("options window: {}, game timer: {}", if ui.options.is_some() { "open" } else { "closed" }, ui.game.timer_game);
            println!("wrote {} sel={:?} unit={:?} pos={:x} viewport={:x}", args[5], ui.game.selection_type, ui.game.unit_selected, ui.game.selection_position, ui.game.viewport_position);
            if let Some(u) = ui.game.unit_selected {
                let un = ui.game.u(u);
                println!("selected unit action={} target_move={:x} target_attack={:x} pos=({},{})", un.action_id, un.target_move, un.target_attack, un.o.position.x >> 8, un.o.position.y >> 8);
            }
        }
        "extract" => {
            // extract NAME out: write one file out of the PAKs.
            std::fs::write(&args[4], vfs.get(&args[3]).expect("no such file")).unwrap();
        }
        "strings" => {
            // strings: every loaded string, by id.
            let st = dune::data::strings::Strings::load(&vfs);
            for i in 0..st.raw.len() {
                println!("{i}: {}", st.text(i as u16));
            }
        }
        "adlinfo" => {
            // adlinfo FILE.ADL: list the tracks of an AdLib file (channel, priority, program).
            let data = vfs.get(&args[3]).expect("no such file").to_vec();
            let mut a = dune::audio::Audio::new(44100);
            a.debug_play_adl(&data, 0xFF);
            for t in 0..120u8 {
                if let Some((ch, prio, prog)) = a.debug_driver().track_info(t) {
                    println!("track {t:3}: program {prog:3} channel {ch} priority {prio}");
                }
            }
        }
        "adl" => {
            // adl FILE.ADL <track> <seconds> out.wav
            let data = vfs.get(&args[3]).expect("no such file").to_vec();
            let track: u8 = args[4].parse().unwrap();
            let secs: f32 = args[5].parse().unwrap();
            let mut a = dune::audio::Audio::new(44100);
            a.debug_play_adl(&data, track);
            let mut buf = vec![0f32; (44100.0 * secs) as usize];
            let mut ended = None;
            for (i, chunk) in buf.chunks_mut(441).enumerate() {
                a.render(chunk);
                if std::env::var("ADLDBG").is_ok() && i % 100 == 0 {
                    let d = a.debug_driver();
                    println!("{:5.1}s active {:010b} ch0 {:?}", i as f32 / 100.0, d.active_mask(), d.channel_ptr(0));
                }
                if ended.is_none() && i > 10 && !a.music_is_playing() {
                    ended = Some(i as f32 / 100.0);
                }
            }
            let peak = buf.iter().fold(0f32, |m, &v| m.max(v.abs()));
            let rms = (buf.iter().map(|v| v * v).sum::<f32>() / buf.len() as f32).sqrt();
            println!("peak {peak:.3} rms {rms:.4} music ended at {ended:?}s");
            write_wav(&args[6], 44100, &buf);
        }
        "voc" => {
            // voc NAME.VOC out.wav
            let v = dune::audio::voc::decode(vfs.get(&args[3]).expect("no such file")).expect("bad voc");
            println!("rate {} samples {}", v.rate, v.samples.len());
            let buf: Vec<f32> = v.samples.iter().map(|&s| (s as f32 - 128.0) / 128.0).collect();
            write_wav(&args[4], v.rate, &buf);
        }
        other => panic!("unknown command {other}"),
    }
}

fn write_wav(path: &str, rate: u32, samples: &[f32]) {
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, out).unwrap();
}
