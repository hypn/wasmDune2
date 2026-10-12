# rsDune2

A browser (WebAssembly) re-implementation of DOS **Dune II**, made only for *play against the computer*
skirmish games. The game logic follows the original closely, using OpenDUNE (a decompilation) as the
specification. The skirmish setup and the base-building AI are new, because the original game never had them.

## Layout

- `crates/dune`: the core (data formats, map, rules, simulation). Plain Rust, no platform code.
- `crates/dune-web`: the WASM front-end, exposing a small C ABI. `web/main.js` drives it; there's no wasm-bindgen.
- `crates/dune-tool`: a native developer tool that dumps maps, tiles, sprites and CPS images to PNG.
- `web/`: the static site. Put the original data files in `web/data/` (never committed).

## Data

- **Full game**: copy the original PAK files (`DUNE.PAK`, `ENGLISH.PAK`, `HARK.PAK`, `ORDOS.PAK`, `ATRE.PAK`, …).
  Both v1.0 and v1.07 data are supported.
- **Shareware**: the 1992 demo PAKs only. The game detects what's missing and degrades gracefully.

## Build & run

    rustup target add wasm32-unknown-unknown
    ./build.sh                       # -> web/pkg/dune.wasm
    ln -s /path/to/dune2 web/data    # or copy the PAKs in
    cd web && python3 -m http.server 8000

Then open http://localhost:8000.

Tool: `cargo run -p dune-tool -- <datadir> map 27 out.png`
