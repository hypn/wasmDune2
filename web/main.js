// Loads the original PAK files from data/, hands them to the WASM core and runs the 60 Hz loop.

const PAKS = [
  'DUNE.PAK', 'ENGLISH.PAK', 'SCENARIO.PAK', 'MENTAT.PAK', 'VOC.PAK', 'MERC.PAK', 'FINALE.PAK',
  'INTRO.PAK', 'INTROVOC.PAK', 'SOUND.PAK', 'ATRE.PAK', 'HARK.PAK', 'ORDOS.PAK',
];

const status = document.getElementById('status');
const canvas = document.getElementById('screen');
const ctx = canvas.getContext('2d');
const setup = document.getElementById('setup');
const form = document.getElementById('setupForm');

function fit() {
  // Integer scale with the original 4:3 pixel aspect (each pixel 1.2x taller).
  const s = Math.max(1, Math.floor(Math.min(innerWidth / 320, innerHeight / 240)));
  canvas.style.width = `${320 * s}px`;
  canvas.style.height = `${240 * s}px`;
}
addEventListener('resize', fit);
fit();

async function main() {
  const { instance } = await WebAssembly.instantiateStreaming(fetch('pkg/dune.wasm'), {});
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

  const loaded = [];
  await Promise.all(PAKS.map(async (name) => {
    const res = await fetch(`data/${name}`);
    if (!res.ok) return;
    loaded.push([name, new Uint8Array(await res.arrayBuffer())]);
  }));
  loaded.sort((a, b) => PAKS.indexOf(a[0]) - PAKS.indexOf(b[0]));
  for (const [name, data] of loaded) {
    const r = withBytes(enc.encode(name), (np, nl) => withBytes(data, (dp, dl) => w.add_pak(np, nl, dp, dl)));
    if (r < 0) throw new Error(readError());
  }
  const variant = w.init();
  if (variant < 0) throw new Error(readError());
  const variantName = variant === 0 ? 'full game' : 'shareware';

  // Audio: the WASM core renders mono samples on demand (OPL2 music/effects plus the voices).
  // The AudioContext can only start from a user gesture, so it is created on START.
  let audioCtx = null;
  function startAudio() {
    if (!audioCtx) {
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
    audioCtx.resume();
    w.audio_init(audioCtx.sampleRate);
  }

  // Setup screen
  let running = false;
  let seedUsed = 0;
  document.getElementById('rnd').onclick = () => { form.seed.value = Math.floor(Math.random() * 32768); };
  const showSetup = () => { setup.style.display = 'flex'; running = false; };
  form.onsubmit = (e) => {
    e.preventDefault();
    let opp = 0;
    for (const h of [0, 1, 2, 4]) if (form[`opp${h}`].checked) opp |= 1 << h;
    const house = +form.house.value;
    opp &= ~(1 << house);
    if (opp === 0) opp = house === 0 ? 2 : 1;
    seedUsed = w.start_skirmish(house, opp, +form.seed.value, +form.scale.value, +form.credits.value, +form.tech.value,
      +form.diff.value, +form.force.value, +form.allied.value, +form.speed.value, +form.enh.value);
    w.set_qol((+form.rclick.value) | (+form.hints.value));
    w.set_debug(form.nofog.checked ? 1 : 0);
    w.set_sim_speed(+form.turbo.value);
    startAudio();
    w.set_audio((+form.music.value) | (+form.sounds.value));
    setup.style.display = 'none';
    running = true;
    last = performance.now();
  };
  showSetup();

  // Input
  const toScreen = (e) => {
    const r = canvas.getBoundingClientRect();
    return [Math.floor((e.clientX - r.left) * 320 / r.width), Math.floor((e.clientY - r.top) * 200 / r.height)];
  };
  canvas.addEventListener('mousemove', (e) => w.mouse_move(...toScreen(e)));
  canvas.addEventListener('mousedown', (e) => { e.preventDefault(); w.mouse_button(e.button, 1, ...toScreen(e)); });
  addEventListener('mouseup', (e) => w.mouse_button(e.button, 0, ...toScreen(e)));
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  const sendKey = (e, down) => {
    if (!running) return;
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
    if (running) {
      acc += Math.min(now - last, 250);
      let n = 0;
      while (acc >= STEP) { acc -= STEP; n++; }
      if (n) w.tick(n);
      const ptr = w.render();
      image.data.set(new Uint8Array(w.memory.buffer, ptr, 320 * 200 * 4));
      ctx.putImageData(image, 0, 0);
      const st = w.game_state();
      status.textContent = `${variantName} · map ${seedUsed}` + (st === 1 ? ' · YOU WON' : st === 2 ? ' · YOU LOST' : '');
    }
    last = now;
    requestAnimationFrame(loop);
  }
  requestAnimationFrame(loop);
}

main().catch((e) => { status.textContent = `Error: ${e.message}`; console.error(e); });
