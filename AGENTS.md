# AGENTS.md — rsDune2

Notes for anyone (human or agent) working on this repo: what the project is, how it is built,
where things stand, and what is left to do.

## Goal

A browser (WebAssembly) version of DOS **Dune II** that is only for **"play against the computer"
skirmish games**: no campaign, mentat briefings, intro or logos. The page starts straight into the
skirmish setup.

- **Stay faithful.** Game logic, UI behaviour and visuals should match the original as closely as possible.
  OpenDUNE (a decompilation) is the specification; port functions line by line and keep the
  original names (snake_cased) so they can be compared side by side.
- **Original bugs:** `GameConfig::dune2_enhanced` mirrors OpenDUNE's flag. When it's on, outright bugs are fixed;
  when it's off, the game behaves exactly like the original.
- **Skirmish additions are new code.** The original had no skirmish mode, and its AI could not build a base.
  These additions live in `game/skirmish.rs` and are hooked into the core in a few marked places. They
  must stay no-ops when `skirmish.enabled` is false.
- **Quality-of-life features are opt-in.** `ui::QolSettings` holds them; their defaults are the original behaviour.
- **Two data variants:**
  - **Full** v1.0/v1.07 data, for local play.
  - **Shareware**, the Nov 1992 demo PAKs, for public hosting.
  - A missing file must never be fatal: degrade gracefully instead.

## Layout

- `crates/dune` (core, no platform code)
  - `data/`: PAK VFS, format80, CPS/ICN/SHP/palette, WSA (format40), strings (.ENG), fonts
  - `tables/`: `generated.rs`, produced by `tools/gen_tables/gen.c` from OpenDUNE's table sources (don't
    hand-edit); `strings.rs`, generated from OpenDUNE `table/strings.h`; `landscape.rs`; `types.rs`
  - `map/landscape.rs`: Map_CreateLandscape (the seed-based terrain generator)
  - `game/`: the simulation. `mod.rs` (Game state, pools, index encoding, tile math, main `tick()`),
    `script.rs` (EMC VM), `script_fns.rs` (all script functions), `pathfinder.rs`, `unit.rs`,
    `structure.rs`, `house.rs`, `team.rs`, `map.rs`, `explosion.rs`, `animation.rs`,
    `skirmish.rs` (new: setup and the AI base-building layer)
  - `audio/`: `opl3.rs` (a port of the Nuked OPL3 FM emulator), `adlib.rs` (a port of the Westwood AdLib
    driver, after ScummVM's Kyra `AdLibDriver`, version-3 behaviour), `voc.rs` (VOC decoder), and `mod.rs`
    (the `Audio` engine: OpenDUNE's sound.c/driver.c semantics plus a mono mixer)
  - `ui/`: `screen.rs` (drawing primitives, GUI_DrawSprite port, fonts), `widget.rs` (the widget
    system and HandleEvents state machine, plus key mapping), `mod.rs` (GameUi: viewport, radar,
    action panel, credits, message line, palette animation, cursor, click procs, hints),
    `factory.rs` (build, construction yard and starport window), `sound.rs` (SoundRequest queue to `Audio`
    and GameLoop_Main's music selection)
- `crates/dune-web`: WASM C-ABI (`init`, `start_skirmish`, `tick`, `render`, input, `audio_init`, `set_audio`,
  `audio_render`)
- `crates/dune-tool`: developer CLI (see below)
- `web/`: `index.html` (setup form) and `main.js` (loader plus the 60 Hz loop). Data goes in `web/data/`, which is
  gitignored; locally it is a symlink to `reference/dune2`.
- `reference/`, gitignored:
  - `OpenDUNE/`: the source used as the spec.
  - `dune2/`: the user's game data. It is **v1.0** data (SHAPES.SHP has 103 sprites, and BTTN sprites are relocated as
    OpenDUNE does), and `ATRE.PAK` is the 30-voice demo copy. The demo installer is `dune2.exe`, a zip
    of DISK1/DISK2.EXE.

## Build, run, test

    rustup target add wasm32-unknown-unknown
    ./build.sh                          # -> web/pkg/dune.wasm
    cd web && python3 -m http.server 8000

Developer tool (`cargo run --release -p dune-tool -- reference/dune2 <cmd>`):
- `list`: PAK contents and the detected variant
- `map <seed> out.png`, `tiles out.png`, `sprites out.png`, `cps NAME.CPS out.png`
- `sim <seed> <minutes>`: headless skirmish; prints per-house credits, power, units and structures.
  `SIMSTEP=<ticks>`, `SIMDBG=1` (dump units and structures), `SIMTILE=x,y`.
- `ui <seed> <ticks> out.png [x,y[,r]@tick ...]`: headless UI with scripted clicks, rendered to PNG.
  `UIDBG=1` and `UIDBG2=1` trace clicks and widget events. `AUDIO=1` runs the audio engine as well and
  reports how many voices, effects and songs started (`AUDIO=2` also lists which voices loaded).
- `adlinfo DUNE1.ADL`: the tracks in an AdLib file (channel, priority). `adl DUNE1.ADL <track> <secs> out.wav`
  renders one track (`ADLDBG=1` traces channels). `voc NAME.VOC out.wav` decodes a voice.

Run `sim` with the **debug** build too: arithmetic overflow panics there and points at porting
mistakes (release builds wrap silently).

Regenerating the tables:
`gcc -std=gnu99 -w -Ireference/OpenDUNE/include -Ireference/OpenDUNE/src tools/gen_tables/gen.c -o gen && ./gen > crates/dune/src/tables/generated.rs`
(the same for `gen_sound.c`, which writes `tables/sound.rs`).

## Conventions and gotchas

- One `tick()` is one 60 Hz timer tick, followed by one in-game iteration of GameLoop_Main. The subsystems
  gate themselves on `timer_game` exactly as the original does.
- C pointers to units or structures become `u16` indices; `Object *` becomes `ObjRef`. Pools keep the
  original "find array" order, which affects iteration order and therefore determinism. `unit_sort()`
  reorders it every frame, as the original does in GUI_DrawScreen.
- Script functions run inside `script_run`, which takes `Ctx`; `script->script--` becomes `ip - 1`.
- Use wrapping/u16 arithmetic wherever the C code relied on it.
- The UI redraws the whole screen every frame; there is no dirty-rect logic. Anything the original
  did as a side effect of drawing still has to run every tick. For example, `draw_action_panel()` decides
  which panel buttons exist, so `GameUi::tick` calls it.
- The widget click procs run inside `Widgets::handle_events` via a callback. `self.input` is replaced by
  a snapshot during that call.
- Modal hints swallow input until they are dismissed, as in the original.
- Audio:
  - The simulation emits `Event`s. `GameUi` turns them, and its own UI sounds, into `SoundRequest`s,
    which `audio_tick` applies to `GameUi::audio` at the end of each tick.
  - `Audio` is `None` headless or until the browser calls `audio_init`.
  - A voice counts as playing until its samples have been *rendered*, so speech queues and music changes
    follow the audio clock, as with the original DSP driver.
  - Music choices use their own LCG (`MusicState::rng`) instead of the game's. The original shared one,
    but sharing would let audio timing change the simulation.
- ADL files use the 120-byte track table followed by data; programs are 250 entries and instruments start
  at program 250; jumps are relative (driver version 3). Music runs on channel 9 and channels 0–5;
  effects use 6–8.

## Status (2026-10-12)

Done:
- Data loaders for every format the game needs, v1.0 and v1.07 handling, and variant detection.
- The core simulation, ported from OpenDUNE: units, structures, houses, teams, EMC scripts, pathfinder,
  map logic (spice, fog, blooms, walls), explosions, animations, palace weapons, starport, reinforcements,
  power and credits.
- Skirmish setup:
  - Random map, with seeds that lack a rocky start for every house skipped.
  - Spread-out starts and MCV starts with optional light or medium forces.
  - Settings for tech level (= campaign 1–9), AI skill (caps the AI build speed through the original
    campaign*20+95 formula), credits and game speed.
  - "Computers allied" (the original rule) or free-for-all; in free-for-all the computers see each other.
  - Win when every enemy has lost all buildings (except turrets) and MCVs; lose when you have.
- AI additions:
  - Build order, power and silo awareness, and structure placement near the base.
  - Starting credits survive until the house has storage, mirroring g_playerCreditsNoSilo.
  - An undeployed MCV counts as a base for the player's no-silo credits.
  - Replacement harvesters.
- In-game UI:
  - Viewport with units, turrets, shadows, the blur effect, explosions and the selection rectangle.
  - Radar, action panel and orders, build button and percentage, repair and upgrade.
  - Credits animation, message line, palette animation, cursors, edge and keyboard scrolling, Tab cycling.
  - Hint dialogs and the factory, construction yard and starport window.
- Bug fixed (enhanced mode only): Map_SearchSpice could return the harvester's own tile, which left
  harvesters stuck.
- Web: an HTML setup form (house, opponents, alliance, seed, size, credits, tech, skill, start force,
  speed, Turbo, bug fixes, right click, hints, DEBUG), the 60 Hz loop, and mouse and keyboard input.
- Options: currently a simple pause toggle. Click Options or press Esc to resume.
- Quality of life (`QolSettings`): "right-click orders" (`right_click_orders`) works like later strategy games:
  right-click an enemy to attack, spice to harvest, anywhere else to move; in target/place mode it
  cancels, and with nothing to order it deselects. The setup form pre-selects it; "Original" is still
  available. The default game speed in the setup form is Fastest (4). Hints are hidden by default
  (`no_hints`; the original shows them).
- Game speed: the original setting (`GameConfig::game_speed`, 0..4) only scales building, movement
  and turning through Tools_AdjustToGameSpeed: Fastest builds 2x faster than Normal. Harvesting, refining,
  firing and scripts are unaffected, as in the original. The setup also has **Turbo**
  (`GameUi::sim_percent`, WASM `set_sim_speed`), which runs more simulation ticks per real tick (default 2x).
  It speeds up everything evenly without changing any rules.
- Audio:
  - AdLib music and effects (OPL emulation) and digitised VOC voices: house voices, unit acknowledgements,
    the feedback speech queue ("enemy unit approaching", ...), explosions and weapons.
  - Ported from OpenDUNE: Voice_PlayAtTile (falls back to the AdLib effect when no voice is loaded or the
    priority is too low), Sound_StartSound, Sound_Output_Feedback/StartSpeech, Music_Play and the in-game
    idle/battle music rotation.
  - The setup form has Music and Sounds on/off.
  - Browser: a ScriptProcessorNode created on START pulls `audio_render`. That costs about 2.4% CPU in WASM.
  - Missing ADL or VOC files are silent: effects fall back as in the original, and missing music is just silence.
- DEBUG options (setup form "DEBUG" section, WASM `set_debug(flags)`): bit 0 = no fog of war, which
  calls `Game::debug_reveal_map()` (unveils every tile for the player). In the tool, use `NOFOG=1`; `QOL=<flags>`
  sets the quality-of-life flags.

## TODO

Next session: the Dune-styled setup screen and options menu, or the audio follow-ups.


- [ ] Audio follow-ups:
  - Without voices, Sound_Output_Feedback shows the message in the viewport (g_viewportMessageText).
    `Audio::sound_output_feedback` returns the string id, but nothing draws it yet.
  - Volume controls (`Audio::music_volume` / `sfx_volume`).
  - Check the mix balance by ear against DOSBox.
  - Consider an AudioWorklet instead of the deprecated ScriptProcessorNode.
- [ ] Replace the HTML form with a Dune-styled in-canvas setup screen (original fonts and borders).
- [ ] Options menu like the original (game controls: speed, hints, auto-scroll), plus restart and
      quit-to-setup. A Mentat help screen is optional.
- [ ] Radar on/off STATIC.WSA animation; the viewport message text; the factory list scroll animation;
      the starport detail rows (out of stock / unable to create).
- [ ] Hold the end-of-game state on screen (win or lose message, stats), then return to setup.
- [ ] AI attack grouping (skirmish layer): gather idle units and attack in waves instead of only
      trickling hunters. Also consider the AI laying concrete.
- [ ] **Differential testing against OpenDUNE**: build OpenDUNE headless, load a campaign scenario in
      both (needs a Scenario_Load port for tests), run N ticks with no input, and compare unit and structure
      state. This is the main fidelity safety net.
- [ ] Shareware (demo) data: test with the demo PAKs extracted from `reference/dune2/dune2.exe`, check
      that missing CHOAM / CREDIT / HARK / ORDOS files degrade gracefully, and possibly limit house choice.
- [ ] More opt-in quality-of-life features: drag-select and multi-select, unit queues.
- [ ] Optional aspect-correct and integer scaling toggle; touch input.
- [ ] Hosting: a public build with only the shareware PAKs.
