//! The game's side of sound: turns simulation events and UI actions into calls on the audio
//! engine, and runs GameLoop_Main's music selection.

use super::GameUi;
use crate::audio::Audio;
use crate::game::get_distance_packed;
use crate::tables::HOUSE_INFO;
use crate::tile::Tile32;

/// A sound the UI wants played; applied to `GameUi::audio` at the end of the tick.
#[derive(Clone, Copy, Debug)]
pub enum SoundRequest {
    /// Sound_Output_Feedback(index)
    Feedback(u16),
    /// Voice_PlayAtTile(voiceID, position): (voice id, volume from the distance)
    VoiceAtTile(u16, u16),
    /// Sound_StartSound(index)
    StartSound(u16),
    /// Driver_Sound_Play(index, 0xFF)
    Effect(u16),
    /// The acknowledgement for a unit order: Sound_StartSound(index) with voices,
    /// Driver_Sound_Play(36) without.
    OrderAck(u16),
}

/// State of GameLoop_Main's music handling.
#[derive(Default)]
pub struct MusicState {
    started: bool,
    /// l_timerNext
    timer_next: u32,
    /// The LCG used for picking songs; kept apart from the simulation's random state so that
    /// whether audio is running cannot change the game.
    rng: crate::rng::Rng,
}

impl GameUi {
    /// Create the audio engine (the browser calls this once the user has interacted).
    pub fn audio_init(&mut self, sample_rate: u32) {
        let mut a = Audio::new(sample_rate);
        let prefix = HOUSE_INFO.get(self.game.player_house_id as usize).map_or(b'A', |h| h.prefix_char);
        a.load(&self.assets.vfs, prefix);
        self.music.rng.seed_lcg(self.game.scenario.map_seed as u16);
        self.music.started = false;
        self.audio = Some(a);
    }

    /// Voice_PlayAtTile's volume: 255 near the top-left of the viewport, fading with distance.
    pub(crate) fn voice_volume(&self, position: Tile32) -> u16 {
        if position.x == 0 && position.y == 0 {
            return 255;
        }
        let d = get_distance_packed(self.game.minimap_position, position.pack()).min(64);
        255 - d * 255 / 80
    }

    /// Apply the queued sound requests, then the music part of GameLoop_Main.
    pub(crate) fn audio_tick(&mut self) {
        let requests = std::mem::take(&mut self.sounds);
        let Some(a) = &mut self.audio else { return };
        for r in requests {
            match r {
                SoundRequest::Feedback(id) => {
                    // Without voices the original shows the message text in the viewport instead.
                    let _ = a.sound_output_feedback(id);
                }
                SoundRequest::VoiceAtTile(id, volume) => a.voice_play_at_tile(id, volume),
                SoundRequest::StartSound(id) => a.sound_start_sound(id),
                SoundRequest::Effect(id) => a.driver_sound_play(id as i16, 0xFF),
                SoundRequest::OrderAck(id) => {
                    if a.has_voices() {
                        a.sound_start_sound(id)
                    } else {
                        a.driver_sound_play(36, 0xFF)
                    }
                }
            }
        }

        let timer_gui = self.game.timer_gui;
        let m = &mut self.music;
        if !m.started {
            // GM_RESTART: start with a random in-game song.
            m.started = true;
            a.music_play(m.rng.lcg_range(0, 8) + 8);
            m.timer_next = timer_gui + 300;
        }
        if !a.voice_is_playing() && !a.sound_start_speech() {
            if !a.music_enabled {
                a.music_play(2);
                self.game.music_in_battle = 0;
            } else if self.game.music_in_battle > 0 {
                a.music_play(m.rng.lcg_range(0, 5) + 17);
                m.timer_next = timer_gui + 300;
                self.game.music_in_battle = -1;
            } else {
                self.game.music_in_battle = 0;
                if timer_gui > m.timer_next && !a.music_is_playing() {
                    a.music_play(m.rng.lcg_range(0, 8) + 8);
                    m.timer_next = timer_gui + 300;
                }
            }
        }
    }
}
