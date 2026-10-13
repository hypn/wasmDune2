# AGENTS.md — wasmDune2

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
- **Shareware data.** This is the Dune II shareware version: `web/data/` and the Docker image hold only
  the Nov 1992 demo PAKs (DUNE, ENGLISH, SOUND, VOC and ATRE.PAK).
  - The page shows "DUNE II SHAREWARE — SKIRMISH" by default.
  - The code still handles full v1.0/v1.07 data in `web/data/` (or `web/data/retail/` with `?retail=true`).
    It detects the full variant when HARK.PAK, ORDOS.PAK and CHOAM.CPS are present. Never commit, bake in or
    publish those files.
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
    `skirmish.rs` (new: setup and the AI base-building layer), `qol.rs` (new: simulation-side quality-of-life options)
  - `audio/`: `opl3.rs` (a port of the Nuked OPL3 FM emulator), `adlib.rs` (a port of the Westwood AdLib
    driver, after ScummVM's Kyra `AdLibDriver`, version-3 behaviour), `voc.rs` (VOC decoder), and `mod.rs`
    (the `Audio` engine: OpenDUNE's sound.c/driver.c semantics plus a mono mixer)
  - `ui/`: `screen.rs` (drawing primitives, GUI_DrawSprite port, fonts), `widget.rs` (the widget
    system and HandleEvents state machine, plus key mapping), `mod.rs` (GameUi: viewport, radar,
    action panel, credits, message line, palette animation, cursor, click procs, hints),
    `factory.rs` (build, construction yard and starport window), `sound.rs` (SoundRequest queue to `Audio`
    and GameLoop_Main's music selection), `setup.rs` (new: the skirmish setup screen), `options.rs` (the Options button's Game controls window),
    `end.rs` (the end of a game: message, stats screen, end window)
- `crates/dune-web`: WASM C-ABI (`init`, `setup_open`/`setup_active`/`setup_done`/`setup_value`, `start_skirmish`, `tick`, `render`,
  input, `audio_init`, `set_audio`, `audio_render`)
- `crates/dune-tool`: developer CLI (see below)
- `web/`: `index.html` (just the canvas) and `main.js` (loader plus the 60 Hz loop). Data goes in `web/data/`, which is
  gitignored. It holds the **shareware** PAKs: ATRE, DUNE, ENGLISH, SOUND and VOC (about 1 MB). The demo's
  INTRO and INTROVOC.PAK are intro-only and not needed. `main.js` fetches DUNE.PAK first and requests MENTAT, MERC,
  HARK and ORDOS.PAK only when it contains CHOAM.CPS (retail), so shareware data never causes a 404.
- `reference/` (gitignored, not part of the repo): a local checkout of OpenDUNE, used as the spec and to
  regenerate the tables. v1.0 data has 103 sprites in SHAPES.SHP, and its BTTN sprites are relocated as OpenDUNE does.

## Build, run, test

    docker compose up -d --build        # build and serve on http://127.0.0.1:8080

Without Docker, for quick iteration (for a debug build, drop `--release` and copy from `debug/`):

    rustup target add wasm32-unknown-unknown
    cargo build --release --target wasm32-unknown-unknown -p dune-web
    mkdir -p web/pkg && cp target/wasm32-unknown-unknown/release/dune_web.wasm web/pkg/dune.wasm
    cd web && python3 -m http.server 8000

Deployment: `docker compose up -d --build` builds the WASM in a Rust stage (target/ cached between builds). The image then runs busybox `httpd`
from `scratch` (about 4 MB, non-root, read-only, no capabilities) on 127.0.0.1:8080. It serves `web/index.html`,
`web/main.js`, the built `pkg/dune.wasm` and whatever is in `web/data/` (the shareware PAKs). `deploy/httpd.conf`
holds the MIME types (wasm must be `application/wasm`). `deploy/nginx.conf` is an example TLS front end with
`Cache-Control: no-cache` and security headers. `web/data/retail/` is never in the image (`.dockerignore`);
`docker-compose.yaml` bind-mounts it read-only at run time for `?retail=true`.

Developer tool (`cargo run --release -p dune-tool -- web/data <cmd>`; the first argument is any directory of PAKs):
- `list`: PAK contents and the detected variant
- `map <seed> out.png`, `tiles out.png`, `sprites out.png`, `cps NAME.CPS out.png`
- `sim <seed> <minutes>`: headless skirmish; prints per-house credits, power, units and structures.
  `SIMSTEP=<ticks>`, `SIMDBG=1` (dump units and structures), `SIMTILE=x,y`, `ALLIED=0` (free-for-all, so the
  computers fight and rebuild among themselves). It reports any `OVERLAP` of structure footprints.
- `ui <seed> <ticks> out.png [x,y[,r]@tick ...]`: headless UI with scripted clicks, rendered to PNG.
  `UIDBG=1` and `UIDBG2=1` trace clicks and widget events. `FACTORY=1` opens a construction yard's build
  window before rendering. `SETUP=1` shows the setup screen (clicks go to it).
  `key:<KeyboardEvent.code>@tick` presses a key (e.g. `key:Escape@50`). `DEPLOY=1` deploys the player's MCV at the start (`DEPLOY=<tick>` at that tick). `AUDIO=1` runs the audio engine as well and
  reports how many voices, effects and songs started (`AUDIO=2` also lists which voices loaded).
- `adlinfo DUNE1.ADL`: the tracks in an AdLib file (channel, priority). `adl DUNE1.ADL <track> <secs> out.wav`
  renders one track (`ADLDBG=1` traces channels). `voc NAME.VOC out.wav` decodes a voice.
- `strings`: every loaded string by id. Diff the full and shareware output to check string ids line up.

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
  - Win when every enemy has lost all buildings (except turrets) and MCVs; lose when you have. Only buildings on the map
    count (as Structure_Find gives the original's check): one a construction yard built but never placed does not. MCVs
    count on the map or carried by a carryall, not while still being built.
  - The end (`ui/end.rs`), after GameLoop_LevelEnd:
    - Music_Play(0), queued speech dropped, the win/lose voice (AWIN/ALOSE.VOC are optional and absent
      from the data we have, so silent), then the original modal "You have successfully completed / failed your mission".
    - GUI_EndStats_Show, for a loss too (the original only after a win): Update_Score (including its time bonus of
      campaign*45 minutes, large at tech 9), time, rank (DUNE2.EXE's 10-entry table; OpenDUNE's has 12), and
      the spice/units/buildings bars drawn a pixel per tick with sounds 52/38, on FAME.CPS with the house crests.
      Shareware has no FAME.CPS: the same layout is drawn in SCREEN.CPS sand with FAME's colours. Its
      DUNE.ENG says "Score: %d%%", so it shows a percent sign.
    - Then (new) a window over the shaded game: "Restart scenario" (same settings and map), "New game" (the setup
      screen with the previous choices) and "Continue game" (or Escape: play on). `main.js` notices the
      setup screen via `setup_active`.
- AI additions:
  - Build order, power and silo awareness, and structure placement near the base.
  - Starting credits survive until the house has storage, mirroring g_playerCreditsNoSilo.
  - An undeployed MCV counts as a base for the player's no-silo credits.
  - Replacement harvesters.
  - Rebuild spots: the original AI rebuilds a lost structure on its old spot without checking it
    (Structure_Place skips the check for computer houses). `skirmish_drop_blocked_rebuilds` forgets a spot
    once something stands on it, so the replacement is placed like any new structure instead of overlapping.
    That includes ground units on (or moving onto) the footprint (`skirmish_footprint_has_units`): Structure_Place
    deletes one unit per tile and leaves any others inside the building. New spots skip such tiles too.
- In-game UI:
  - Viewport with units, turrets, shadows, the blur effect, explosions and the selection rectangle.
  - Radar, action panel and orders, build button and percentage, repair and upgrade.
  - Credits animation, message line, palette animation, cursors, edge and keyboard scrolling, Tab cycling.
  - Hint dialogs and the factory, construction yard and starport window.
- Starport stock: skirmish has no scenario `[CHOAM]` section, so `skirmish_starport_stock` seeds
  `starport_available` with the original's last scenario of the chosen tech level (scenario 22 at tech 9).
- Bugs fixed (enhanced mode only):
- Porting traps found so far:
  - Tile.groundTileID is a 9-bit field in the original: values copied from g_mapTileID must drop its 0x8000
    "built on" flag (`& 0x1FF`), or the tile is invalid and draws black (shot concrete, stopped structure animations).
  - Turrets: Script_Structure_Fire reads its target from script variable 2 (and Unit_UntargetMe clears it), but
    BUILD.EMC keeps the target in a local and passes it to Fire on the stack. Nothing ever writes variable 2, so taken
    literally turrets never fire. `ss_fire` uses the stack argument when variable 2 is empty (and it is still a valid object).
  - Turrets also pass RotateTurret the result of GetDirection (a direction, orientation8 << 5), while the original
    looks its argument up as an encoded index: "nothing", tile (0, 0), so turrets swung towards the top left of the map.
    `ss_rotate_turret` takes a non-index argument as a direction. GetDirection measures from the tile centre (+0x80),
    as the original's assembly does (OpenDUNE's C uses the corner).
  - Map_SearchSpice could return the harvester's own tile, which left harvesters stuck.
  - Structure_SetState restarted the structure's animation even when the state didn't change. The
    outpost/silo script in BUILD.EMC ends in a top-level return that underflows the stack and reloads the script,
    and it sets its state again about every 45 ticks, so the radar dish only flicked between its first two frames.
    OpenDUNE behaves the same.
- Web: the 60 Hz loop, mouse and keyboard input, and the setup screen.
  - The setup screen (`ui/setup.rs`) is drawn and handled by the game in the canvas, laid out like the original
    option windows but dressed in SCREEN.CPS art (marbled sand, MENTAT-style buttons, bold blue lettering).
    It offers house, opponents, map seed (digits type one, clicking rolls one; `?seed=N` preselects one, and starting a game writes the
    seed it played into the address bar as `?seed=`), map size, computer skill, music,
    sounds and fog of war (off = the DEBUG no-fog option), plus a radar-style preview of the map as it would start. Enter or START starts;
    `main.js` then reads `setup_value` and calls `start_skirmish`.
  - Hidden settings (the `FIXED` object in `main.js`; add a button in `setup.rs` to offer one again): alliance =
    computers allied (1), credits = 1500, tech level = 9, starting units = MCV + light force (1), game speed =
    Fastest (4), Turbo = 2× (200), bug fixes = on (1), right click = orders (5), hints = hide (2), edge scrolling =
    also right edge (8), H = home (16), harvesters return to their field (32),
    repairs during upgrades (64).
- Options (`ui/options.rs`): the Options button opens the original "Game controls" window directly (music, sounds,
  game speed, hints, auto scroll; game strings, the original layout narrowed to 200 pixels) over the shaded, paused game, styled like the
  setup screen. The rest of the original options window (load, save, restart, pick another house) is left out.
  "Previous" became "Continue game"; it and Escape close the window.
- Quality of life (`QolSettings`): "right-click orders" (`right_click_orders`) works like later strategy games:
  right-click an enemy to attack, spice to harvest, anywhere else to move; in target/place mode it
  cancels, and with nothing to order it deselects. The web front-end turns it on. Its game speed is
  Fastest (4). Hints are hidden by default
  (`no_hints`; the original shows them).
- QoL `GameConfig::harvester_return` (set_qol bit 5, on in the web front-end; `qol.rs`): the player's harvesters
  remember the tile they last harvested (`Unit::last_harvest`). Leaving the refinery, or searching for spice away from
  it, they go back there while seen spice is left within 4 tiles, else to the nearest spice the player has seen, and only
  when none is seen anywhere to what Map_SearchSpice finds. A carryall may also fly them back when the field is over
  8 tiles from the refinery. The original only remembers the spot when a carryall brought the harvester in; computer
  harvesters keep that.
- QoL `GameConfig::repair_while_upgrading` (set_qol bit 6, on in the web front-end): a structure can repair and
  upgrade at the same time (each pays its own cost per structure tick), starting an upgrade keeps a repair going, and
  during an upgrade the Repair/Upgrade button offers Repair (or Repairing) while the structure is damaged. In the
  original the button reads "Upgrading" then, and clicking it to repair stops the upgrade.
- Structure decay is the original's: every Tools_AdjustToGameSpeed(10800, 5400, 21600, inverse) game ticks (5400 at
  Fastest, so 45 real seconds with Turbo 2x) structures not fully on concrete lose the house's degradingAmount, down to
  half their hitpoints. A deployed MCV's construction yard is never on concrete.
- QoL `home_key` (set_qol bit 4, on in the web front-end): H centres the view on the player's first (oldest)
  construction yard, or without one on their MCV; the selection is kept. Only when no panel button claims H
  (with a harvester selected, H stays the original "Harvest" shortcut).
- Mouse and edge scrolling:
  - The browser tracks the mouse over the whole page and clamps it to the 320x200 screen, as the DOS
    mouse was confined. Pushing past the canvas therefore reaches the original 2-pixel left and bottom scroll strips.
  - QoL `screen_edge_scroll` (set_qol bit 3, on in the web front-end) also makes the far right screen edge scroll right.
    The original's right strip is x 240..249, between the viewport and the sidebar. The game is shown the mouse
    at x=245 during widget handling while it hovers at x >= 318 with no button held.
  - The tool's `HOVER=x,y` parks the mouse for testing, and `ui` prints the viewport position.
- Game speed: the original setting (`GameConfig::game_speed`, 0..4) only scales building, movement
  and turning through Tools_AdjustToGameSpeed: Fastest builds 2x faster than Normal. Harvesting, refining,
  firing and scripts are unaffected, as in the original. The web front-end also uses **Turbo**
  (`GameUi::sim_percent`, WASM `set_sim_speed`), which runs more simulation ticks per real tick (default 2x).
  It speeds up everything evenly without changing any rules.
- Audio:
  - AdLib music and effects (OPL emulation) and digitised VOC voices: house voices, unit acknowledgements,
    the feedback speech queue ("enemy unit approaching", ...), explosions and weapons.
  - Ported from OpenDUNE: Voice_PlayAtTile (falls back to the AdLib effect when no voice is loaded or the
    priority is too low), Sound_StartSound, Sound_Output_Feedback/StartSpeech, Music_Play and the in-game
    idle/battle music rotation.
  - The setup screen has Music and Sounds on/off.
  - Browser: a ScriptProcessorNode, created on the first click or key press, pulls `audio_render`. That costs about 2.4% CPU in WASM.
  - Missing ADL or VOC files are silent: effects fall back as in the original, and missing music is just silence.
- Shareware data (detected when HARK.PAK, ORDOS.PAK or CHOAM.CPS is missing):
  - Units, structures, terrain and scripts all work.
  - The demo DUNE.ENG lacks one string and has a few others in different places. `Strings::remap_demo`
    rebuilds the v1.0 id layout.
  - The demo has no build-window art (CHOAM.CPS, CHOAMSHP.SHP, WSA previews, GRAYRMAP.TBL), and none is
    added: the build stays shareware-only.
    - The window draws a framed background and labelled buttons instead.
    - The preview (no WSAs) is a sand backdrop with the item icon at 2x.
    - `dune-tool extract NAME out` pulls a file out of the PAKs.
  - Only Atreides voices exist, so the setup screen allows only Atreides as the player. Opponents are unrestricted.
  - Only 4 ADL music files exist (DUNE0/1/3/12). The song picker re-rolls past missing songs.
- DEBUG options (setup screen "Fog of war OFF", WASM `set_debug(flags)`): bit 0 = no fog of war, which
  calls `Game::debug_reveal_map()` (unveils every tile for the player) and sets
  `GameUi::debug_full_radar`. That flag draws the minimap as if the radar were up; it is display only and leaves
  the house's radar state, voice and feedback untouched. In the tool, use `NOFOG=1` (both); `QOL=<flags>`
  sets the quality-of-life flags.
