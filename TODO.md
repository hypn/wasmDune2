# TODO

Suggested next: the Dune-styled setup screen and options menu, or the audio follow-ups.

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
- [ ] Shareware: play a full game by hand on the demo data (starport window, palace, high tech).
- [ ] More opt-in quality-of-life features: drag-select and multi-select, unit queues.
- [ ] Optional aspect-correct and integer scaling toggle; touch input.
