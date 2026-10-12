//! Sound: digitised voices (Sound Blaster DSP) and AdLib music and effects, mixed to mono f32.
//!
//! The public functions mirror OpenDUNE's `audio/sound.c` (Music_Play, Voice_PlayAtTile,
//! Sound_StartSound, Sound_Output_Feedback, Sound_StartSpeech) and the parts of
//! `audio/driver.c` they use. The front-end pulls samples with `render`; a voice counts as
//! "playing" until its samples have been rendered, as with the original DSP driver.

pub mod adlib;
pub mod opl3;
pub mod voc;

use std::collections::HashMap;

use crate::data::pak::Vfs;
use crate::tables::sound::{FEEDBACK, MUSICS, VOICES};
use adlib::{AdlibDriver, CALLBACKS_PER_SECOND};
use voc::Voc;

const NUM_SPEECH_PARTS: usize = 5;

struct PlayingVoice {
    voice: usize,
    /// Position in source samples, 16.16 fixed point.
    pos: u64,
    step: u64,
}

pub struct Audio {
    sample_rate: u32,
    adlib: AdlibDriver,
    adlib_acc: u32,
    /// .ADL files by name, e.g. "DUNE1.ADL".
    adl_files: HashMap<String, Vec<u8>>,
    /// g_voiceData, decoded. Index = VOICES index. '?' entries are preloaded for the player's house.
    voices: Vec<Option<Voc>>,
    playing: Option<PlayingVoice>,
    /// s_spokenWords
    spoken_words: [u16; NUM_SPEECH_PARTS],
    /// s_currentVoicePriority
    current_voice_priority: i16,
    /// Driver_Voice_Play's l_currentPriority
    driver_voice_priority: i16,
    /// Music_Play's currentMusicID
    current_music_id: u16,
    /// s_currentMusic
    current_music: Option<&'static str>,
    /// g_gameConfig.music
    pub music_enabled: bool,
    /// g_gameConfig.sounds
    pub sounds_enabled: bool,
    pub music_volume: f32,
    pub sfx_volume: f32,
    /// Counters for testing: voices started, AdLib effects started, songs started.
    pub stats: [u32; 3],
}

impl Audio {
    pub fn new(sample_rate: u32) -> Audio {
        Audio {
            sample_rate,
            adlib: AdlibDriver::new(sample_rate),
            adlib_acc: 0,
            adl_files: HashMap::new(),
            voices: vec![None; VOICES.len()],
            playing: None,
            spoken_words: [0xFFFF; NUM_SPEECH_PARTS],
            current_voice_priority: 0,
            driver_voice_priority: -1,
            current_music_id: 0,
            current_music: None,
            music_enabled: true,
            sounds_enabled: true,
            music_volume: 1.0,
            sfx_volume: 1.0,
            stats: [0; 3],
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Voice_LoadVoices(playerHouse) for the in-game voice set, plus the AdLib music files.
    /// Missing files are skipped (the shareware data lacks many of them).
    pub fn load(&mut self, vfs: &Vfs, house_prefix: u8) {
        for name in vfs.names() {
            if name.ends_with(".ADL") {
                if let Some(d) = vfs.get(name) {
                    self.adl_files.insert(name.clone(), d.to_vec());
                }
            }
        }
        let load = |name: &str| vfs.get(name).and_then(voc::decode);
        for (i, &(pattern, _)) in VOICES.iter().enumerate() {
            let fill = |p: &str, c: u8| p.replace("%c", &(c as char).to_string());
            self.voices[i] = match pattern.as_bytes()[0] {
                b'%' => load(&fill(pattern, house_prefix)),
                b'+' => {
                    let name = fill(&pattern[1..], b'Z');
                    // Some files are named without the house prefix.
                    load(&name).or_else(|| load(&name[1..]))
                }
                // Loaded on demand by Sound_StartSound, with the player's house prefix.
                b'?' => load(&fill(&pattern[1..], house_prefix)),
                // Intro-only voices.
                b'-' | b'/' => None,
                _ => load(pattern),
            };
        }
    }

    pub fn voice_loaded(&self, index: usize) -> bool {
        self.voices.get(index).is_some_and(|v| v.is_some())
    }

    pub fn voices_loaded(&self) -> usize {
        self.voices.iter().filter(|v| v.is_some()).count()
    }

    pub fn has_voices(&self) -> bool {
        self.voices.iter().any(|v| v.is_some())
    }

    // -------------------------------------------------------------------------------------
    // Driver layer
    // -------------------------------------------------------------------------------------

    /// Driver_Voice_IsPlaying
    pub fn voice_is_playing(&self) -> bool {
        self.playing.is_some()
    }

    /// Driver_Voice_Play(data, priority); `voice` None stops the current voice.
    fn driver_voice_play(&mut self, voice: Option<usize>, priority: i16) {
        if !self.sounds_enabled {
            return;
        }
        let priority = match voice {
            None => 0x100,
            Some(_) => priority.min(0xFF),
        };
        if !self.voice_is_playing() {
            self.driver_voice_priority = -1;
        }
        if priority < self.driver_voice_priority {
            return;
        }
        self.playing = None;
        let Some(v) = voice else { return };
        self.driver_voice_priority = priority;
        if let Some(voc) = &self.voices[v] {
            let step = ((voc.rate as u64) << 16) / self.sample_rate as u64;
            self.playing = Some(PlayingVoice { voice: v, pos: 0, step });
            self.stats[0] += 1;
        }
    }

    /// Driver_Voice_Play(NULL, 0xFF): stop the current voice (e.g. when the options menu opens).
    pub fn stop_voice(&mut self) {
        self.driver_voice_play(None, 0xFF);
    }

    /// Driver_Voice_Stop
    fn driver_voice_stop(&mut self) {
        self.playing = None;
    }

    /// Driver_Sound_Play(index, volume): an AdLib sound effect from the loaded music file.
    pub fn driver_sound_play(&mut self, index: i16, volume: i16) {
        if !(0..120).contains(&index) {
            return;
        }
        if !self.sounds_enabled && index > 1 {
            return;
        }
        self.adlib.play(index as u8, (volume & 0xFF) as u8);
        self.stats[1] += 1;
    }

    /// Driver_Music_IsPlaying
    pub fn music_is_playing(&self) -> bool {
        self.adlib.is_music_playing()
    }

    // -------------------------------------------------------------------------------------
    // sound.c
    // -------------------------------------------------------------------------------------

    /// Whether the music file for `music_id` is present (the shareware data has only a few).
    pub fn music_available(&self, music_id: u16) -> bool {
        MUSICS.get(music_id as usize).is_some_and(|(f, _)| self.adl_files.contains_key(&format!("{}.ADL", f.to_ascii_uppercase())))
    }

    /// Music_Play(musicID)
    pub fn music_play(&mut self, music_id: u16) {
        if music_id == 0xFFFF || music_id >= 38 || music_id == self.current_music_id {
            return;
        }
        self.current_music_id = music_id;
        let (file, index) = MUSICS[music_id as usize];
        if Some(file) != self.current_music {
            self.current_music = Some(file);
            self.driver_voice_play(None, 0xFF);
            match self.adl_files.get(&format!("{}.ADL", file.to_ascii_uppercase())) {
                Some(d) => {
                    let d = d.clone();
                    self.adlib.load(&d);
                }
                None => self.adlib.load(&[]),
            }
        }
        // Driver_Music_Play
        if index > 120 || !self.music_enabled {
            return;
        }
        self.adlib.stop_music();
        self.adlib.play(index as u8, 0xFF);
        self.stats[2] += 1;
    }

    /// Voice_PlayAtTile(voiceID, position), with the volume already derived from the distance.
    pub fn voice_play_at_tile(&mut self, voice_id: u16, volume: u16) {
        if voice_id >= 120 || !self.sounds_enabled {
            return;
        }
        let index = crate::tables::sound::VOICE_MAPPING[voice_id as usize];
        if index != 0xFFFF
            && self.voices.get(index as usize).is_some_and(|v| v.is_some())
            && VOICES[index as usize].1 as i16 >= self.current_voice_priority
        {
            self.current_voice_priority = VOICES[index as usize].1 as i16;
            self.driver_voice_play(Some(index as usize), self.current_voice_priority);
        } else {
            self.driver_sound_play(voice_id as i16, volume as i16);
        }
    }

    /// Sound_StartSound(index)
    pub fn sound_start_sound(&mut self, index: u16) {
        if index == 0xFFFF || !self.sounds_enabled || index as usize >= VOICES.len() {
            return;
        }
        if (VOICES[index as usize].1 as i16) < self.current_voice_priority {
            return;
        }
        self.current_voice_priority = VOICES[index as usize].1 as i16;
        if self.voices[index as usize].is_some() {
            self.driver_voice_play(Some(index as usize), 0xFF);
        }
    }

    /// Sound_Output_Feedback(index). Returns the message string to show in the viewport when
    /// no voices are available (the original's g_viewportMessageText).
    pub fn sound_output_feedback(&mut self, index: u16) -> Option<u16> {
        if index == 0xFFFF {
            return None;
        }
        if index == 0xFFFE {
            self.spoken_words = [0xFFFF; NUM_SPEECH_PARTS];
            self.driver_voice_stop();
            self.current_voice_priority = 0;
            return None;
        }
        let fb = FEEDBACK.get(index as usize)?;
        if !self.has_voices() || !self.sounds_enabled {
            self.driver_sound_play(fb.2 as i16, 0xFF);
            return Some(fb.1);
        }
        if self.spoken_words[0] == 0xFFFF {
            self.spoken_words = fb.0;
        }
        self.sound_start_speech();
        None
    }

    /// Sound_StartSpeech
    pub fn sound_start_speech(&mut self) -> bool {
        if !self.sounds_enabled {
            return false;
        }
        if self.voice_is_playing() {
            return true;
        }
        self.current_voice_priority = 0;
        if self.spoken_words[0] == 0xFFFF {
            return false;
        }
        self.sound_start_sound(self.spoken_words[0]);
        self.spoken_words.copy_within(1.., 0);
        self.spoken_words[NUM_SPEECH_PARTS - 1] = 0xFFFF;
        true
    }

    /// Switch music on/off (the original's options menu calls Music_Play(0) when switching off).
    pub fn set_music_enabled(&mut self, on: bool) {
        if self.music_enabled == on {
            return;
        }
        self.music_enabled = on;
        if !on {
            self.adlib.stop_music();
        }
    }

    // -------------------------------------------------------------------------------------
    // Output
    // -------------------------------------------------------------------------------------

    /// Render mono samples in -1..1.
    pub fn render(&mut self, out: &mut [f32]) {
        for o in out.iter_mut() {
            self.adlib_acc += CALLBACKS_PER_SECOND;
            if self.adlib_acc >= self.sample_rate {
                self.adlib_acc -= self.sample_rate;
                self.adlib.callback();
            }
            let fm = self.adlib.opl.generate_resampled() as f32 / 32768.0 * 2.0 * self.music_volume;
            let mut dig = 0.0;
            if let Some(p) = &mut self.playing {
                let samples = &self.voices[p.voice].as_ref().unwrap().samples;
                let i = (p.pos >> 16) as usize;
                if i + 1 >= samples.len() {
                    self.playing = None;
                } else {
                    let f = (p.pos & 0xFFFF) as f32 / 65536.0;
                    let a = samples[i] as f32 - 128.0;
                    let b = samples[i + 1] as f32 - 128.0;
                    dig = (a + (b - a) * f) / 128.0 * 0.5 * self.sfx_volume;
                    p.pos += p.step;
                }
            }
            *o = (fm + dig).clamp(-1.0, 1.0);
        }
    }

    /// Render without output (keeps the timing of voices and music going).
    pub fn skip(&mut self, samples: usize) {
        let mut buf = [0f32; 256];
        let mut left = samples;
        while left > 0 {
            let n = left.min(buf.len());
            self.render(&mut buf[..n]);
            left -= n;
        }
    }

    /// Debug helper: play an ADL track directly.
    pub fn debug_play_adl(&mut self, file: &[u8], track: u8) {
        self.adlib.load(file);
        self.adlib.play(track, 0xFF);
    }

    pub fn debug_driver(&self) -> &AdlibDriver {
        &self.adlib
    }
}
