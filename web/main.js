// Loads the original PAK files from data/ (or data/retail/ with ?retail=true), hands them to the WASM core and runs the 60 Hz loop.

// The shareware demo has only the base PAKs. The rest are requested only when DUNE.PAK is the
// retail one (it contains CHOAM.CPS), so shareware data never causes a 404.
const BASE_PAKS = ['DUNE.PAK', 'ENGLISH.PAK', 'VOC.PAK', 'SOUND.PAK', 'ATRE.PAK'];
const FULL_PAKS = ['MENTAT.PAK', 'MERC.PAK', 'HARK.PAK', 'ORDOS.PAK'];
const PAKS = ['DUNE.PAK', 'ENGLISH.PAK', 'MENTAT.PAK', 'VOC.PAK', 'MERC.PAK', 'SOUND.PAK', 'ATRE.PAK', 'HARK.PAK', 'ORDOS.PAK'];

// ?retail=true loads the full (retail) game from data/retail/ instead of the shareware data in
// data/, falling back to data/ when data/retail/ is missing. For private convenience only:
// data/retail/ is never built into the image or committed, it has to be mounted or symlinked in
// (see README.md). The page presents itself as the shareware version unless the data turns out
// to be the full game.
const RETAIL = ['true', '1', 'yes'].includes(new URLSearchParams(location.search).get('retail'));

// ?seed=N preselects the map seed on the setup screen. Starting a game puts the seed it played
// into the address bar, so the same map can be played again by reloading or sharing the link.
const SEED_PARAM = Number.parseInt(new URLSearchParams(location.search).get('seed'), 10);
const SEED = Number.isInteger(SEED_PARAM) && SEED_PARAM >= 0 && SEED_PARAM <= 32767 ? SEED_PARAM : 27;

const status = document.getElementById('status');
const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');

// Settings the setup screen does not offer (they used to be on the HTML form): see AGENTS.md.
const FIXED = {
  credits: 1500, tech: 9, force: 1, allied: 1, speed: 4, enhanced: 1,
  // right click orders, no hints, right edge scrolls, H = home, harvesters return to their field,
  // repairs go on during upgrades
  turbo: 200, qol: 5 | 2 | 8 | 16 | 32 | 64 | 128,
};

function fit() {
  // Integer scale with the original 4:3 pixel aspect (each pixel 1.2x taller).
  const s = Math.max(1, Math.floor(Math.min(innerWidth / 320, innerHeight / 240)));
  canvas.style.width = `${320 * s}px`;
  canvas.style.height = `${240 * s}px`;
}
addEventListener('resize', fit);
fit();

// Whether a PAK's directory (u32 offset + NUL-terminated name entries, ended by the first file's
// data or a zero offset) lists `file`.
function pakContains(pak, file) {
  const view = new DataView(pak.buffer, pak.byteOffset, pak.byteLength);
  let pos = 0, end = pak.length;
  while (pos + 4 < end) {
    const offset = view.getUint32(pos, true);
    if (offset === 0) break;
    end = Math.min(end, offset);
    let name = '';
    for (pos += 4; pos < end && pak[pos] !== 0; pos++) name += String.fromCharCode(pak[pos]);
    if (name.toUpperCase() === file) return true;
    pos++;
  }
  return false;
}

async function main() {
  const { instance } = await WebAssembly.instantiateStreaming(fetch('pkg/dune.wasm', { cache: 'no-cache' }), {});
  const w = instance.exports;
  const enc = new TextEncoder();
  const readError = () => new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.error_ptr(), w.error_len()));
  const withBytes = (bytes, f) => {
    const p = w.alloc(bytes.length);
    new Uint8Array(w.memory.buffer, p, bytes.length).set(bytes);
    const r = f(p, bytes.length);
    w.dealloc(p, bytes.length);
    return r;
  };

  const loadPaks = async (dir) => {
    const loaded = [];
    const fetchAll = (names) => Promise.all(names.map(async (name) => {
      // no-cache: revalidate, so swapping web/data (full <-> shareware) is never hidden by the
      // browser's heuristic caching of these 1992-dated files.
      const res = await fetch(`${dir}/${name}`, { cache: 'no-cache' });
      if (!res.ok) return;
      loaded.push([name, new Uint8Array(await res.arrayBuffer())]);
    }));
    // DUNE.PAK first: without it there is nothing to load, and it decides which other PAKs exist.
    await fetchAll(['DUNE.PAK']);
    if (loaded.length === 0) return loaded;
    const full = pakContains(loaded[0][1], 'CHOAM.CPS');
    await fetchAll([...BASE_PAKS.slice(1), ...(full ? FULL_PAKS : [])]);
    return loaded;
  };
  let loaded = RETAIL ? await loadPaks('data/retail') : [];
  const retailMissing = RETAIL && loaded.length === 0;
  if (!RETAIL || retailMissing) loaded = await loadPaks('data');
  loaded.sort((a, b) => PAKS.indexOf(a[0]) - PAKS.indexOf(b[0]));
  for (const [name, data] of loaded) {
    const r = withBytes(enc.encode(name), (np, nl) => withBytes(data, (dp, dl) => w.add_pak(np, nl, dp, dl)));
    if (r < 0) throw new Error(readError());
  }
  const variant = w.init();
  if (variant < 0) throw new Error(readError());
  const variantName = variant === 0 ? 'full game' : 'shareware';
  if (variant === 0) document.title = 'Dune II';
  if (retailMissing) console.warn('No retail data in data/retail/, so this is the shareware version.');

  // Audio: the WASM core renders mono samples on demand (OPL2 music/effects plus the voices).
  // The AudioContext can only start from a user gesture, so it is created on the first click or key.
  let audioCtx = null;
  function createAudio() {
    if (audioCtx) { audioCtx.resume(); return; }
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    audioCtx = new AC();
    const node = audioCtx.createScriptProcessor(2048, 0, 2);
    node.onaudioprocess = (e) => {
      const n = e.outputBuffer.length;
      const left = e.outputBuffer.getChannelData(0);
      const right = e.outputBuffer.getChannelData(1);
      if (!running) { left.fill(0); right.fill(0); return; }
      const ptr = w.audio_render(n);
      left.set(new Float32Array(w.memory.buffer, ptr, n));
      right.set(left);
    };
    node.connect(audioCtx.destination);
  }

  // Setup screen: drawn and handled by the game itself (like the original option windows);
  // START there starts the skirmish from its choices plus the FIXED settings.
  let running = false;
  let seedUsed = 0;
  w.setup_open(variant === 1 ? 1 : 0, SEED);
  function startGame() {
    // Read every choice first: start_skirmish closes the setup screen.
    const v = ((values) => (i) => values[i])([0, 1, 2, 3, 4, 5, 6, 7].map((i) => w.setup_value(i)));
    seedUsed = w.start_skirmish(v(0), v(1), v(2), v(3), FIXED.credits, FIXED.tech, v(4), FIXED.force,
      FIXED.allied, FIXED.speed, FIXED.enhanced);
    if (seedUsed >= 0) {
      const url = new URL(location.href);
      url.searchParams.set('seed', seedUsed);
      history.replaceState(null, '', url);
    }
    w.set_qol(FIXED.qol);
    w.set_debug(v(7) ? 1 : 0);
    w.set_sim_speed(FIXED.turbo);
    if (audioCtx) {
      w.audio_init(audioCtx.sampleRate);
      w.set_audio(v(5) | (v(6) << 1));
    }
    running = true;
  }

  // Input
  const toScreen = (e) => {
    const r = canvas.getBoundingClientRect();
    // Clamped to the screen, like the DOS mouse: pushing past the canvas edge keeps the cursor on
    // the edge, so the 2-pixel edge scroll strips are easy to hit.
    const x = Math.floor((e.clientX - r.left) * 320 / r.width);
    const y = Math.floor((e.clientY - r.top) * 200 / r.height);
    return [Math.max(0, Math.min(319, x)), Math.max(0, Math.min(199, y))];
  };
  // Track the mouse over the whole page, not just the canvas.
  addEventListener('mousemove', (e) => w.mouse_move(...toScreen(e)));
  // Switching to another window must not leave the cursor parked on a scroll edge.
  addEventListener('blur', () => w.mouse_move(120, 100));
  canvas.addEventListener('mousedown', (e) => { e.preventDefault(); createAudio(); w.mouse_button(e.button, 1, ...toScreen(e)); });
  addEventListener('mouseup', (e) => w.mouse_button(e.button, 0, ...toScreen(e)));
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  const sendKey = (e, down) => {
    if (down) createAudio();
    withBytes(enc.encode(e.code), (p, l) => w.key(p, l, down));
    if (e.code.startsWith('Arrow') || e.code === 'Tab' || e.code === 'Space') e.preventDefault();
  };
  addEventListener('keydown', (e) => sendKey(e, 1));
  addEventListener('keyup', (e) => sendKey(e, 0));

  // 60 Hz game loop, rendering on animation frames.
  const image = new ImageData(320, 200);
  let last = performance.now(), acc = 0;
  const STEP = 1000 / 60;
  function loop(now) {
    acc += Math.min(now - last, 250);
    let n = 0;
    while (acc >= STEP) { acc -= STEP; n++; }
    if (n) w.tick(n);
    // The end of a game can go back to the setup screen (and restart from it straight away).
    if (running && w.setup_active()) running = false;
    if (!running && w.setup_done()) startGame();
    const ptr = w.render();
    image.data.set(new Uint8Array(w.memory.buffer, ptr, 320 * 200 * 4));
    ctx.putImageData(image, 0, 0);
    if (running) {
      const st = w.game_state();
      status.textContent = `${variantName} · map ${seedUsed}` + (st === 1 ? ' · YOU WON' : st === 2 ? ' · YOU LOST' : '');
    } else {
      status.textContent = '';
    }
    last = now;
    requestAnimationFrame(loop);
  }
  requestAnimationFrame(loop);
}

main().catch((e) => { status.textContent = `Error: ${e.message}`; console.error(e); });
