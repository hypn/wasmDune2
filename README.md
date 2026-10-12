# wasmDune2

A WebAssembly re-implementation of DOS **Dune II** for skirmish games against the computer. The game logic
follows the original closely, using [OpenDUNE](https://github.com/OpenDUNE/OpenDUNE) as the specification. The
skirmish setup and base-building AI are new.

It ships with the data from the freely distributable **Dune II shareware demo** (November 1992).

## Layout

- `crates/dune`: the core: data formats, map, rules, simulation and audio. Plain Rust, no platform code.
- `crates/dune-web`: the WASM front end, a small C ABI driven by `web/main.js`.
- `crates/dune-tool`: a native developer tool (renders maps and sprites, music to WAV, headless games).
- `web/`: the static site; game data lives in `web/data/`.
- `Dockerfile`, `docker-compose.yaml`, `deploy/`: build and serve the site in a hardened container.

## Game data

`web/data/` contains the shareware PAKs: `DUNE`, `ENGLISH`, `SOUND`, `VOC` and `ATRE.PAK`. With this data
you play as Atreides (the only house with voices in the demo), and the build window uses simplified graphics.

The retail game (v1.0 or v1.07) is also supported for private use: place its `DUNE`, `ENGLISH`, `SOUND`, `VOC`,
`ATRE`, `HARK`, `ORDOS` and `MENTAT.PAK` (optionally `MERC.PAK`) in `web/data/`. File names must be upper case.
Retail files are copyrighted; don't host them publicly.

## Build & run

With Docker:

    docker compose up -d --build                 # http://127.0.0.1:8080, hardened, for use behind a reverse proxy

    docker build -t wasmdune2-web .              # or, for a quick local run
    docker run --rm --init -p 8080:8080 wasmdune2-web

`deploy/nginx.conf` is an example TLS front end.

Without Docker:

    rustup target add wasm32-unknown-unknown
    cargo build --release --target wasm32-unknown-unknown -p dune-web
    mkdir -p web/pkg && cp target/wasm32-unknown-unknown/release/dune_web.wasm web/pkg/dune.wasm
    cd web && python3 -m http.server 8000        # http://localhost:8000

See [AGENTS.md](AGENTS.md) for architecture notes, conventions and the developer tool.
