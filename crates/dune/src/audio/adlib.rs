//! The Westwood AdLib driver that plays Dune II's .ADL music and sound effects.
//! A Rust port of ScummVM's Kyra `AdLibDriver` (GPL), using the "version 3" behaviour
//! (250 programs, relative jumps), which matches Dune II's ADL files.

use super::opl3::Opl3;

pub const CALLBACKS_PER_SECOND: u32 = 72;
const NUM_PROGRAMS: usize = 250;

const REG_OFFSET: [u8; 9] = [0x00, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x10, 0x11, 0x12];
const FREQ_TABLE: [u16; 12] = [0x0134, 0x0147, 0x015A, 0x016F, 0x0184, 0x019C, 0x01B4, 0x01CE, 0x01E9, 0x0207, 0x0225, 0x0246];

const PITCH_BEND_TABLES: [[u8; 32]; 14] = [
    [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21],
    [0x00, 0x01, 0x02, 0x03, 0x04, 0x06, 0x07, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x22, 0x24],
    [0x00, 0x01, 0x02, 0x03, 0x04, 0x06, 0x08, 0x09, 0x0A, 0x0C, 0x0D, 0x0E, 0x0F, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x24, 0x25, 0x26],
    [0x00, 0x01, 0x02, 0x03, 0x04, 0x06, 0x08, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x1A, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x23, 0x25, 0x27, 0x28],
    [0x00, 0x01, 0x02, 0x03, 0x04, 0x06, 0x08, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x11, 0x13, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1B, 0x1D, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x28, 0x2A],
    [0x00, 0x01, 0x02, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1B, 0x1D, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x25, 0x27, 0x29, 0x2B, 0x2D],
    [0x00, 0x01, 0x02, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13, 0x15, 0x16, 0x17, 0x18, 0x1A, 0x1C, 0x1E, 0x21, 0x24, 0x25, 0x26, 0x27, 0x29, 0x2B, 0x2D, 0x2F, 0x30],
    [0x00, 0x01, 0x02, 0x04, 0x06, 0x08, 0x0A, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x13, 0x15, 0x18, 0x19, 0x1A, 0x1C, 0x1D, 0x1F, 0x21, 0x23, 0x25, 0x26, 0x27, 0x29, 0x2B, 0x2D, 0x2F, 0x30, 0x32],
    [0x00, 0x01, 0x02, 0x04, 0x06, 0x08, 0x0A, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x14, 0x17, 0x1A, 0x19, 0x1A, 0x1C, 0x1E, 0x20, 0x22, 0x25, 0x28, 0x29, 0x2A, 0x2B, 0x2D, 0x2F, 0x31, 0x33, 0x35],
    [0x00, 0x01, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0E, 0x0F, 0x10, 0x12, 0x14, 0x16, 0x18, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x20, 0x22, 0x24, 0x26, 0x29, 0x2A, 0x2C, 0x2E, 0x30, 0x32, 0x34, 0x36, 0x39],
    [0x00, 0x01, 0x03, 0x05, 0x07, 0x09, 0x0B, 0x0E, 0x0F, 0x10, 0x12, 0x14, 0x16, 0x19, 0x1B, 0x1E, 0x1F, 0x21, 0x23, 0x25, 0x27, 0x29, 0x2B, 0x2D, 0x2E, 0x2F, 0x31, 0x32, 0x34, 0x36, 0x39, 0x3C],
    [0x00, 0x01, 0x03, 0x05, 0x07, 0x0A, 0x0C, 0x0F, 0x10, 0x11, 0x13, 0x15, 0x17, 0x19, 0x1B, 0x1E, 0x1F, 0x20, 0x22, 0x24, 0x26, 0x28, 0x2B, 0x2E, 0x2F, 0x30, 0x32, 0x34, 0x36, 0x39, 0x3C, 0x3F],
    [0x00, 0x02, 0x04, 0x06, 0x08, 0x0B, 0x0D, 0x10, 0x11, 0x12, 0x14, 0x16, 0x18, 0x1B, 0x1E, 0x21, 0x22, 0x23, 0x25, 0x27, 0x29, 0x2C, 0x2F, 0x32, 0x33, 0x34, 0x36, 0x38, 0x3B, 0x34, 0x41, 0x44],
    [0x00, 0x02, 0x04, 0x06, 0x08, 0x0B, 0x0D, 0x11, 0x12, 0x13, 0x15, 0x17, 0x1A, 0x1D, 0x20, 0x23, 0x24, 0x25, 0x27, 0x29, 0x2C, 0x2F, 0x32, 0x35, 0x36, 0x37, 0x39, 0x3B, 0x3E, 0x41, 0x44, 0x47],
];

fn unk_table2(entry: usize) -> &'static [u8] {
    const T1: [u8; 130] = {
        let mut t = [0u8; 130];
        let mut i = 0;
        while i < 130 {
            t[i] = 0x50 - (i / 2) as u8;
            i += 1;
        }
        t
    };
    const T2: [u8; 128] = {
        let mut t = [0u8; 128];
        let mut i = 0;
        while i < 128 {
            t[i] = i as u8;
            i += 1;
        }
        t[95] = 0x6F;
        t
    };
    const T3: [u8; 130] = {
        let mut t = [0u8; 130];
        let mut i = 0;
        while i < 130 {
            t[i] = 0x40 - (i / 3) as u8;
            i += 1;
        }
        t
    };
    match entry {
        0 | 2 => &T1,
        4 => &T3,
        _ => &T2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Effect {
    #[default]
    None,
    Slide,
    Vibrato,
    Secondary1,
}

#[derive(Clone, Copy, Default)]
struct Channel {
    op_extra_level2: u8,
    dataptr: Option<usize>,
    duration: u8,
    repeat_counter: u8,
    base_octave: i8,
    priority: u8,
    dataptr_stack_pos: u8,
    dataptr_stack: [usize; 4],
    base_note: i8,
    slide_tempo: u8,
    slide_timer: u8,
    slide_step: i16,
    vibrato_step: i16,
    vibrato_step_range: u8,
    vibrato_steps_countdown: u8,
    vibrato_num_steps: u8,
    vibrato_delay: u8,
    vibrato_tempo: u8,
    vibrato_timer: u8,
    vibrato_delay_countdown: u8,
    op_extra_level1: u8,
    spacing2: u8,
    base_freq: u8,
    tempo: u8,
    timer: u8,
    reg_ax: u8,
    reg_bx: u8,
    primary_effect: Effect,
    secondary_effect: Effect,
    fractional_spacing: u8,
    op_level1: u8,
    op_level2: u8,
    op_extra_level3: u8,
    two_chan: u8,
    spacing1: u8,
    duration_randomness: u8,
    secondary_effect_tempo: u8,
    secondary_effect_timer: u8,
    secondary_effect_size: i8,
    secondary_effect_pos: i8,
    secondary_effect_regbase: u8,
    secondary_effect_data: u16,
    tempo_reset: u8,
    raw_note: u8,
    pitch_bend: i8,
    volume_modifier: u8,
}

#[derive(Clone, Copy, Default)]
struct QueueEntry {
    data: Option<usize>,
    id: u8,
    volume: u8,
}

/// Number of parameter bytes per opcode (ScummVM's _parserOpcodeTable).
const OPCODE_VALUES: [usize; 75] = [
    1, 2, 1, 1, 2, 2, 0, 1, 0, 1, 2, 2, 1, 5, 1, 1, 1, 3, 0, 1, 0, 4, 0, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 2, 2, 1,
    1, 1, 0, 0, 1, 0, 2, 0, 0, 0, 1, 0, 0, 1, 1, 0, 2, 0, 9, 1, 0, 2, 2, 2, 1, 1, 2, 0,
];

pub struct AdlibDriver {
    pub opl: Opl3,
    /// Track table (the first 120 bytes of the file): sound id per track.
    tracks: [u8; 120],
    data: Vec<u8>,
    cur_channel: i32,
    sound_trigger: u8,
    rnd: u16,
    beat_divider: u8,
    beat_div_cnt: u8,
    callback_timer: u8,
    beat_counter: u8,
    beat_waiting: u8,
    op_level_bd: u8,
    op_level_hh: u8,
    op_level_sd: u8,
    op_level_tt: u8,
    op_level_cy: u8,
    op_extra_level1_hh: u8,
    op_extra_level2_hh: u8,
    op_extra_level1_cy: u8,
    op_extra_level2_cy: u8,
    op_extra_level2_tt: u8,
    op_extra_level1_tt: u8,
    op_extra_level1_sd: u8,
    op_extra_level2_sd: u8,
    op_extra_level1_bd: u8,
    op_extra_level2_bd: u8,
    queue: [QueueEntry; 16],
    program_start_timeout: i32,
    queue_start: usize,
    queue_end: usize,
    retry_sounds: bool,
    sfx_pointer: Option<usize>,
    sfx_priority: u8,
    sfx_velocity: u8,
    channels: [Channel; 10],
    vibrato_and_am_depth_bits: u8,
    rhythm_section_bits: u8,
    cur_reg_offset: u8,
    tempo: u8,
    music_volume: u8,
    sfx_volume: u8,
}

fn advance(timer: &mut u8, tempo: u8) -> bool {
    let old = *timer;
    *timer = timer.wrapping_add(tempo);
    *timer < old
}

impl AdlibDriver {
    pub fn new(sample_rate: u32) -> AdlibDriver {
        let mut d = AdlibDriver {
            opl: Opl3::new(sample_rate),
            tracks: [0xFF; 120],
            data: Vec::new(),
            cur_channel: 0,
            sound_trigger: 0,
            rnd: 0x1234,
            beat_divider: 0,
            beat_div_cnt: 0,
            callback_timer: 0xFF,
            beat_counter: 0,
            beat_waiting: 0,
            op_level_bd: 0,
            op_level_hh: 0,
            op_level_sd: 0,
            op_level_tt: 0,
            op_level_cy: 0,
            op_extra_level1_hh: 0,
            op_extra_level2_hh: 0,
            op_extra_level1_cy: 0,
            op_extra_level2_cy: 0,
            op_extra_level2_tt: 0,
            op_extra_level1_tt: 0,
            op_extra_level1_sd: 0,
            op_extra_level2_sd: 0,
            op_extra_level1_bd: 0,
            op_extra_level2_bd: 0,
            queue: [QueueEntry::default(); 16],
            program_start_timeout: 0,
            queue_start: 0,
            queue_end: 0,
            retry_sounds: false,
            sfx_pointer: None,
            sfx_priority: 0,
            sfx_velocity: 0,
            channels: [Channel::default(); 10],
            vibrato_and_am_depth_bits: 0,
            rhythm_section_bits: 0,
            cur_reg_offset: 0,
            tempo: 0,
            music_volume: 0xFF,
            sfx_volume: 0xFF,
        };
        d.reset_adlib_state();
        d
    }

    fn write_opl(&mut self, reg: u8, val: u8) {
        self.opl.write_reg(reg as u16, val);
    }

    fn byte(&self, p: usize) -> u8 {
        self.data.get(p).copied().unwrap_or(0)
    }

    fn check_offset(&self, p: Option<usize>, n: i64) -> Option<usize> {
        let p = p? as i64;
        let np = p + n;
        (np >= 0 && np <= self.data.len() as i64).then_some(np as usize)
    }

    fn get_program(&self, prog: usize) -> Option<usize> {
        if prog >= self.data.len() / 2 {
            return None;
        }
        let off = u16::from_le_bytes([self.data[2 * prog], self.data[2 * prog + 1]]) as usize;
        (off != 0 && off < self.data.len()).then_some(off)
    }

    fn get_instrument(&self, id: usize) -> Option<usize> {
        self.get_program(NUM_PROGRAMS + id)
    }

    /// Load an .ADL file (stops everything first, like SoundPC_v1::internalLoadFile).
    pub fn load(&mut self, file: &[u8]) {
        self.stop_all_channels();
        if file.len() < 120 {
            self.data.clear();
            return;
        }
        self.tracks.copy_from_slice(&file[..120]);
        self.data = file[120..].to_vec();
        self.queue = [QueueEntry::default(); 16];
        self.queue_start = 0;
        self.queue_end = 0;
        self.sfx_pointer = None;
    }

    pub fn has_data(&self) -> bool {
        !self.data.is_empty()
    }

    /// Play track `track` (via the file's track table) at `volume` (0xFF = full / no adjust).
    pub fn play(&mut self, track: u8, volume: u8) {
        let id = self.tracks.get(track as usize).copied().unwrap_or(0xFF);
        if id == 0xFF || self.data.is_empty() {
            return;
        }
        self.start_sound(id as usize, volume);
    }

    fn start_sound(&mut self, track: usize, volume: u8) {
        let Some(p) = self.get_program(track) else { return };
        self.queue[self.queue_end] = QueueEntry { data: Some(p), id: track as u8, volume };
        self.queue_end = (self.queue_end + 1) & 15;
    }

    pub fn is_channel_playing(&self, ch: usize) -> bool {
        self.channels[ch].dataptr.is_some()
    }

    /// Is any music channel (0..5 or the control channel) busy?
    pub fn is_music_playing(&self) -> bool {
        (0..6).chain(9..10).any(|c| self.channels[c].dataptr.is_some())
    }

    /// Driver_Music_Stop: silence the music channels (0..5 and the control channel 9).
    pub fn stop_music(&mut self) {
        for ch in (0..6).chain(9..10) {
            self.cur_channel = ch as i32;
            self.channels[ch].priority = 0;
            self.channels[ch].dataptr = None;
            if ch != 9 {
                self.note_off(ch);
            }
        }
    }

    /// Debug: (channel, priority) of the program a track starts, if any.
    pub fn track_info(&self, track: u8) -> Option<(u8, u8, usize)> {
        let id = *self.tracks.get(track as usize)?;
        if id == 0xFF {
            return None;
        }
        let p = self.get_program(id as usize)?;
        Some((self.byte(p), self.byte(p + 1), id as usize))
    }

    /// Debug: a channel's current data offset.
    pub fn channel_ptr(&self, c: usize) -> Option<usize> {
        self.channels[c].dataptr
    }

    /// Debug: bitmask of channels with a program running.
    pub fn active_mask(&self) -> u16 {
        (0..10).filter(|&c| self.channels[c].dataptr.is_some()).fold(0, |m, c| m | 1 << c)
    }

    pub fn stop_all_channels(&mut self) {
        for ch in 0..=9 {
            self.cur_channel = ch as i32;
            self.channels[ch].priority = 0;
            self.channels[ch].dataptr = None;
            if ch != 9 {
                self.note_off(ch);
            }
        }
        self.retry_sounds = false;
        self.queue_start = 0;
        self.queue_end = 0;
        self.queue[0] = QueueEntry::default();
        self.program_start_timeout = 0;
    }

    /// The 72 Hz driver callback.
    pub fn callback(&mut self) {
        if self.program_start_timeout != 0 {
            self.program_start_timeout -= 1;
        } else {
            self.setup_programs();
        }
        self.execute_programs();
        if advance(&mut self.callback_timer, self.tempo) {
            self.beat_div_cnt = self.beat_div_cnt.wrapping_sub(1);
            if self.beat_div_cnt == 0 {
                self.beat_div_cnt = self.beat_divider;
                self.beat_counter = self.beat_counter.wrapping_add(1);
            }
        }
    }

    fn setup_programs(&mut self) {
        let entry = self.queue[self.queue_start];
        if self.queue_start == self.queue_end && entry.data.is_none() {
            return;
        }
        let mut retry = QueueEntry::default();
        if entry.id == 0 {
            self.retry_sounds = true;
        } else if self.retry_sounds {
            retry = entry;
        }
        self.queue[self.queue_start].data = None;
        self.queue_start = (self.queue_start + 1) & 15;
        let Some(ptr) = entry.data else { return };
        if self.check_offset(Some(ptr), 2).is_none() {
            return;
        }
        let chan = self.byte(ptr) as usize;
        if chan > 9 || (chan < 9 && self.check_offset(Some(ptr), 4).is_none()) {
            return;
        }
        self.adjust_sfx_data(ptr, entry.volume);
        let priority = self.byte(ptr + 1);
        if priority >= self.channels[chan].priority {
            self.init_channel(chan);
            let c = &mut self.channels[chan];
            c.priority = priority;
            c.dataptr = Some(ptr + 2);
            c.tempo = 0xFF;
            c.timer = 0xFF;
            c.duration = 1;
            c.volume_modifier = if chan <= 5 { self.music_volume } else { self.sfx_volume };
            self.init_adlib_channel(chan);
            self.program_start_timeout = 2;
            retry = QueueEntry::default();
        }
        if retry.data.is_some() {
            self.start_sound(retry.id as usize, retry.volume);
        }
    }

    fn adjust_sfx_data(&mut self, ptr: usize, volume: u8) {
        if let Some(p) = self.sfx_pointer.take() {
            if p + 3 < self.data.len() {
                self.data[p + 1] = self.sfx_priority;
                self.data[p + 3] = self.sfx_velocity;
            }
        }
        if self.byte(ptr) == 9 || ptr + 3 >= self.data.len() {
            return;
        }
        self.sfx_pointer = Some(ptr);
        self.sfx_priority = self.data[ptr + 1];
        self.sfx_velocity = self.data[ptr + 3];
        if volume != 0xFF {
            // version >= 3
            let new_val = (((self.data[ptr + 3] as i32 + 63) * volume as i32) >> 8) & 0xFF;
            self.data[ptr + 3] = (63 - new_val) as u8;
            self.data[ptr + 1] = ((self.data[ptr + 1] as i32 * volume as i32) >> 8) as u8;
        }
    }

    fn execute_programs(&mut self) {
        let mut ch: i32 = 9;
        while ch >= 0 {
            self.cur_channel = ch;
            let c = ch as usize;
            if self.channels[c].dataptr.is_none() {
                ch -= 1;
                continue;
            }
            self.cur_reg_offset = if c == 9 { 0 } else { REG_OFFSET[c] };
            if self.channels[c].tempo_reset != 0 {
                self.channels[c].tempo = self.tempo;
            }
            let mut result = 1;
            let tempo = self.channels[c].tempo;
            if advance(&mut self.channels[c].timer, tempo) {
                self.channels[c].duration = self.channels[c].duration.wrapping_sub(1);
                if self.channels[c].duration != 0 {
                    if self.channels[c].duration == self.channels[c].spacing2 {
                        self.note_off(c);
                    }
                    if self.channels[c].duration == self.channels[c].spacing1 && c != 9 {
                        self.note_off(c);
                    }
                } else {
                    result = 0;
                }
            }
            while result == 0 {
                let Some(p) = self.channels[c].dataptr else { break };
                let mut opcode = 0xFFu8;
                if self.check_offset(Some(p), 1).is_some() && p < self.data.len() {
                    opcode = self.data[p];
                    self.channels[c].dataptr = Some(p + 1);
                }
                let p = self.channels[c].dataptr.unwrap();
                if opcode & 0x80 != 0 {
                    let op = ((opcode & 0x7F) as usize).min(OPCODE_VALUES.len() - 1);
                    let n = OPCODE_VALUES[op];
                    if self.check_offset(Some(p), n as i64).is_none() || p + n > self.data.len() {
                        result = self.update_stop_channel(c);
                        break;
                    }
                    let values: Vec<u8> = self.data[p..p + n].to_vec();
                    self.channels[c].dataptr = Some(p + n);
                    result = self.run_opcode(op, c, &values);
                } else {
                    if p >= self.data.len() {
                        result = self.update_stop_channel(c);
                        break;
                    }
                    let duration = self.data[p];
                    self.channels[c].dataptr = Some(p + 1);
                    self.setup_note(opcode, c, false);
                    self.note_on(c);
                    self.setup_duration(duration, c);
                    result = (duration != 0) as i32;
                }
            }
            if result == 1 {
                let pe = self.channels[c].primary_effect;
                if pe != Effect::None {
                    self.run_effect(pe, c);
                }
                let se = self.channels[c].secondary_effect;
                if se != Effect::None {
                    self.run_effect(se, c);
                }
            }
            ch -= 1;
        }
    }

    fn run_effect(&mut self, e: Effect, c: usize) {
        match e {
            Effect::Slide => self.primary_effect_slide(c),
            Effect::Vibrato => self.primary_effect_vibrato(c),
            Effect::Secondary1 => self.secondary_effect1(c),
            Effect::None => {}
        }
    }

    fn reset_adlib_state(&mut self) {
        self.rnd = 0x1234;
        self.write_opl(0x01, 0x20);
        self.write_opl(0x08, 0x00);
        self.write_opl(0xBD, 0x00);
        self.init_channel(9);
        for ch in (0..9).rev() {
            self.write_opl(0x40 + REG_OFFSET[ch], 0x3F);
            self.write_opl(0x43 + REG_OFFSET[ch], 0x3F);
            self.init_channel(ch);
        }
    }

    fn init_channel(&mut self, c: usize) {
        let backup = self.channels[c].op_extra_level2;
        self.channels[c] = Channel { op_extra_level2: backup, tempo: 0xFF, spacing1: 1, ..Default::default() };
    }

    fn note_off(&mut self, c: usize) {
        if self.cur_channel >= 9 || c >= 9 {
            return;
        }
        if self.rhythm_section_bits != 0 && self.cur_channel >= 6 {
            return;
        }
        self.channels[c].reg_bx &= 0xDF;
        let v = self.channels[c].reg_bx;
        self.write_opl(0xB0 + self.cur_channel as u8, v);
    }

    fn init_adlib_channel(&mut self, chan: usize) {
        if chan >= 9 {
            return;
        }
        if self.rhythm_section_bits != 0 && chan >= 6 {
            return;
        }
        let off = REG_OFFSET[chan];
        self.write_opl(0x60 + off, 0xFF);
        self.write_opl(0x63 + off, 0xFF);
        self.write_opl(0x80 + off, 0xFF);
        self.write_opl(0x83 + off, 0xFF);
        self.write_opl(0xB0 + chan as u8, 0x00);
        self.write_opl(0xB0 + chan as u8, 0x20);
    }

    fn get_random_nr(&mut self) -> u16 {
        self.rnd = self.rnd.wrapping_add(0x9248);
        let low = self.rnd & 7;
        self.rnd >>= 3;
        self.rnd |= low << 13;
        self.rnd
    }

    fn setup_duration(&mut self, duration: u8, c: usize) {
        if self.channels[c].duration_randomness != 0 {
            let r = self.get_random_nr() as u8;
            let ch = &mut self.channels[c];
            ch.duration = duration.wrapping_add(r & ch.duration_randomness);
            return;
        }
        let ch = &mut self.channels[c];
        if ch.fractional_spacing != 0 {
            ch.spacing2 = (duration >> 3).wrapping_mul(ch.fractional_spacing);
        }
        ch.duration = duration;
    }

    fn setup_note(&mut self, raw_note: u8, c: usize, flag: bool) {
        if self.cur_channel >= 9 {
            return;
        }
        let ch = &mut self.channels[c];
        ch.raw_note = raw_note;
        let mut note = (raw_note & 0x0F) as i8 as i32 + ch.base_note as i32;
        let mut octave = ((raw_note as i32 + ch.base_octave as i32) >> 4) & 0x0F;
        if note >= 12 {
            octave += note / 12;
            note %= 12;
        } else if note < 0 {
            let octaves = -(note + 1) / 12 + 1;
            octave -= octaves;
            note += 12 * octaves;
        }
        let mut freq = FREQ_TABLE[note as usize].wrapping_add(ch.base_freq as u16);
        if ch.pitch_bend != 0 || flag {
            let idx = ((raw_note & 0x0F) as usize).min(11);
            if ch.pitch_bend >= 0 {
                freq = freq.wrapping_add(PITCH_BEND_TABLES[idx + 2][(ch.pitch_bend as usize).min(31)] as u16);
            } else {
                freq = freq.wrapping_sub(PITCH_BEND_TABLES[idx][((-(ch.pitch_bend as i32)) as usize).min(31)] as u16);
            }
        }
        ch.reg_ax = (freq & 0xFF) as u8;
        ch.reg_bx = (ch.reg_bx & 0x20) | ((octave as u8) << 2) | ((freq >> 8) & 0x03) as u8;
        let (ax, bx) = (ch.reg_ax, ch.reg_bx);
        let cc = self.cur_channel as u8;
        self.write_opl(0xA0 + cc, ax);
        self.write_opl(0xB0 + cc, bx);
    }

    fn setup_instrument(&mut self, reg_offset: u8, ptr: usize, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        if ptr + 11 > self.data.len() {
            return;
        }
        let d: Vec<u8> = self.data[ptr..ptr + 11].to_vec();
        self.write_opl(0x20 + reg_offset, d[0]);
        self.write_opl(0x23 + reg_offset, d[1]);
        let cc = self.cur_channel as u8;
        self.write_opl(0xC0 + cc, d[2]);
        self.channels[c].two_chan = d[2] & 1;
        self.write_opl(0xE0 + reg_offset, d[3]);
        self.write_opl(0xE3 + reg_offset, d[4]);
        self.channels[c].op_level1 = d[5];
        self.channels[c].op_level2 = d[6];
        let l1 = self.calculate_op_level1(c);
        let l2 = self.calculate_op_level2(c);
        self.write_opl(0x40 + reg_offset, l1);
        self.write_opl(0x43 + reg_offset, l2);
        self.write_opl(0x60 + reg_offset, d[7]);
        self.write_opl(0x63 + reg_offset, d[8]);
        self.write_opl(0x80 + reg_offset, d[9]);
        self.write_opl(0x83 + reg_offset, d[10]);
    }

    fn note_on(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        self.channels[c].reg_bx |= 0x20;
        let bx = self.channels[c].reg_bx;
        self.write_opl(0xB0 + self.cur_channel as u8, bx);
        let ch = &mut self.channels[c];
        let shift = 9 - (ch.vibrato_step_range as i8).clamp(0, 9);
        let freq = (((ch.reg_bx as u16) << 8) | ch.reg_ax as u16) & 0x3FF;
        ch.vibrato_step = ((freq >> shift) & 0xFF) as i16;
        ch.vibrato_delay_countdown = ch.vibrato_delay;
    }

    fn adjust_volume(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let off = REG_OFFSET[self.cur_channel as usize];
        let l2 = self.calculate_op_level2(c);
        self.write_opl(0x43 + off, l2);
        if self.channels[c].two_chan != 0 {
            let l1 = self.calculate_op_level1(c);
            self.write_opl(0x40 + off, l1);
        }
    }

    fn primary_effect_slide(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let tempo = self.channels[c].slide_tempo;
        if !advance(&mut self.channels[c].slide_timer, tempo) {
            return;
        }
        let ch = &mut self.channels[c];
        let mut freq = (((ch.reg_bx & 0x03) as i16) << 8) | ch.reg_ax as i16;
        let mut octave = ch.reg_bx & 0x1C;
        let note_on = ch.reg_bx & 0x20;
        freq += ch.slide_step.clamp(-0x3FF, 0x3FF);
        if ch.slide_step >= 0 && freq >= 734 {
            freq >>= 1;
            if freq & 0x3FF == 0 {
                freq += 1;
            }
            octave = octave.wrapping_add(4);
        } else if ch.slide_step < 0 && freq < 388 {
            if freq < 0 {
                freq = 0;
            }
            freq <<= 1;
            if freq & 0x3FF == 0 {
                freq -= 1;
            }
            octave = octave.wrapping_sub(4);
        }
        ch.reg_ax = (freq & 0xFF) as u8;
        ch.reg_bx = note_on | (octave & 0x1C) | ((freq >> 8) & 0x03) as u8;
        let (ax, bx) = (ch.reg_ax, ch.reg_bx);
        let cc = self.cur_channel as u8;
        self.write_opl(0xA0 + cc, ax);
        self.write_opl(0xB0 + cc, bx);
    }

    fn primary_effect_vibrato(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        if self.channels[c].vibrato_delay_countdown != 0 {
            self.channels[c].vibrato_delay_countdown -= 1;
            return;
        }
        let tempo = self.channels[c].vibrato_tempo;
        if advance(&mut self.channels[c].vibrato_timer, tempo) {
            let ch = &mut self.channels[c];
            ch.vibrato_steps_countdown = ch.vibrato_steps_countdown.wrapping_sub(1);
            if ch.vibrato_steps_countdown == 0 {
                ch.vibrato_step = -ch.vibrato_step;
                ch.vibrato_steps_countdown = ch.vibrato_num_steps;
            }
            let mut freq = (((ch.reg_bx as u16) << 8) | ch.reg_ax as u16) & 0x3FF;
            freq = freq.wrapping_add(ch.vibrato_step as u16);
            ch.reg_ax = (freq & 0xFF) as u8;
            ch.reg_bx = (ch.reg_bx & 0xFC) | (freq >> 8) as u8;
            let (ax, bx) = (ch.reg_ax, ch.reg_bx);
            let cc = self.cur_channel as u8;
            self.write_opl(0xA0 + cc, ax);
            self.write_opl(0xB0 + cc, bx);
        }
    }

    fn secondary_effect1(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let tempo = self.channels[c].secondary_effect_tempo;
        if advance(&mut self.channels[c].secondary_effect_timer, tempo) {
            let ch = &mut self.channels[c];
            ch.secondary_effect_pos -= 1;
            if ch.secondary_effect_pos < 0 {
                ch.secondary_effect_pos = ch.secondary_effect_size;
            }
            let idx = ch.secondary_effect_data as usize + ch.secondary_effect_pos as usize;
            let reg = ch.secondary_effect_regbase.wrapping_add(self.cur_reg_offset);
            let v = self.byte(idx);
            self.write_opl(reg, v);
        }
    }

    fn calculate_op_level1(&self, c: usize) -> u8 {
        let ch = &self.channels[c];
        let mut value = ch.op_level1 & 0x3F;
        if ch.two_chan != 0 {
            value = value.wrapping_add(ch.op_extra_level1).wrapping_add(ch.op_extra_level2);
            let mut level3 = ((ch.op_extra_level3 ^ 0x3F) as u16) * ch.volume_modifier as u16;
            if level3 != 0 {
                level3 += 0x3F;
                level3 >>= 8;
            }
            value = value.wrapping_add((level3 ^ 0x3F) as u8);
        }
        if value & 0x80 != 0 {
            value = 0;
        }
        value = value.min(0x3F);
        if ch.volume_modifier == 0 {
            value = 0x3F;
        }
        value | (ch.op_level1 & 0xC0)
    }

    fn calculate_op_level2(&self, c: usize) -> u8 {
        let ch = &self.channels[c];
        let mut value = (ch.op_level2 & 0x3F).wrapping_add(ch.op_extra_level1).wrapping_add(ch.op_extra_level2);
        let mut level3 = ((ch.op_extra_level3 ^ 0x3F) as u16) * ch.volume_modifier as u16;
        if level3 != 0 {
            level3 += 0x3F;
            level3 >>= 8;
        }
        value = value.wrapping_add((level3 ^ 0x3F) as u8);
        if value & 0x80 != 0 {
            value = 0;
        }
        value = value.min(0x3F);
        if ch.volume_modifier == 0 {
            value = 0x3F;
        }
        value | (ch.op_level2 & 0xC0)
    }

    fn check_value(v: i32) -> u8 {
        v.clamp(0, 0x3F) as u8
    }

    fn update_stop_channel(&mut self, c: usize) -> i32 {
        self.channels[c].priority = 0;
        if self.cur_channel != 9 {
            self.note_off(c);
        }
        self.channels[c].dataptr = None;
        2
    }

    fn run_opcode(&mut self, op: usize, c: usize, v: &[u8]) -> i32 {
        let le16 = |v: &[u8], i: usize| i16::from_le_bytes([v[i], v[i + 1]]);
        let be16 = |v: &[u8], i: usize| u16::from_be_bytes([v[i], v[i + 1]]);
        match op {
            0 => {
                self.channels[c].repeat_counter = v[0];
                0
            }
            1 => {
                self.channels[c].repeat_counter = self.channels[c].repeat_counter.wrapping_sub(1);
                if self.channels[c].repeat_counter != 0 {
                    let add = le16(v, 0) as i64;
                    if let Some(p) = self.check_offset(self.channels[c].dataptr, add) {
                        self.channels[c].dataptr = Some(p);
                    }
                }
                0
            }
            2 => {
                if v[0] == 0xFF {
                    return 0;
                }
                let Some(ptr) = self.get_program(v[0] as usize) else { return 0 };
                if self.check_offset(Some(ptr), 2).is_none() {
                    return 0;
                }
                let chan = self.byte(ptr) as usize;
                let priority = self.byte(ptr + 1);
                if chan > 9 {
                    return 0;
                }
                if priority >= self.channels[chan].priority {
                    let backup = self.channels[c].dataptr;
                    self.program_start_timeout = 2;
                    self.init_channel(chan);
                    let ch2 = &mut self.channels[chan];
                    ch2.priority = priority;
                    ch2.dataptr = Some(ptr + 2);
                    ch2.tempo = 0xFF;
                    ch2.timer = 0xFF;
                    ch2.duration = 1;
                    ch2.volume_modifier = if chan <= 5 { self.music_volume } else { self.sfx_volume };
                    self.init_adlib_channel(chan);
                    self.channels[c].dataptr = backup;
                }
                0
            }
            3 => {
                self.channels[c].spacing1 = v[0];
                0
            }
            4 => {
                let add = le16(v, 0) as i64;
                self.channels[c].dataptr = self.check_offset(self.channels[c].dataptr, add);
                if self.channels[c].dataptr.is_none() {
                    return self.update_stop_channel(c);
                }
                0
            }
            5 => {
                let add = le16(v, 0) as i64;
                if self.channels[c].dataptr_stack_pos >= 4 {
                    return 0;
                }
                let cur = self.channels[c].dataptr.unwrap_or(0);
                let pos = self.channels[c].dataptr_stack_pos as usize;
                self.channels[c].dataptr_stack[pos] = cur;
                self.channels[c].dataptr_stack_pos += 1;
                match self.check_offset(Some(cur), add) {
                    Some(p) => self.channels[c].dataptr = Some(p),
                    None => {
                        self.channels[c].dataptr_stack_pos -= 1;
                        self.channels[c].dataptr = Some(cur);
                    }
                }
                0
            }
            6 => {
                if self.channels[c].dataptr_stack_pos == 0 {
                    return self.update_stop_channel(c);
                }
                self.channels[c].dataptr_stack_pos -= 1;
                let pos = self.channels[c].dataptr_stack_pos as usize;
                self.channels[c].dataptr = Some(self.channels[c].dataptr_stack[pos]);
                0
            }
            7 => {
                self.channels[c].base_octave = v[0] as i8;
                0
            }
            9 => {
                self.setup_duration(v[0], c);
                self.note_off(c);
                (v[0] != 0) as i32
            }
            10 => {
                self.write_opl(v[0], v[1]);
                0
            }
            11 => {
                self.setup_note(v[0], c, false);
                self.setup_duration(v[1], c);
                (v[1] != 0) as i32
            }
            12 => {
                self.channels[c].base_note = v[0] as i8;
                0
            }
            13 => {
                let ch = &mut self.channels[c];
                ch.secondary_effect_timer = v[0];
                ch.secondary_effect_tempo = v[0];
                ch.secondary_effect_size = v[1] as i8;
                ch.secondary_effect_pos = v[1] as i8;
                ch.secondary_effect_regbase = v[2];
                ch.secondary_effect_data = (u16::from_le_bytes([v[3], v[4]]) as i32 - 191) as u16;
                ch.secondary_effect = Effect::Secondary1;
                let start = ch.secondary_effect_data as i16 as i32 + ch.secondary_effect_size as i32;
                if start < 0 || start >= self.data.len() as i32 {
                    self.channels[c].secondary_effect = Effect::None;
                }
                0
            }
            14 => {
                if v[0] > 9 {
                    return 0;
                }
                let backup = self.channels[c].dataptr;
                let ch2 = &mut self.channels[v[0] as usize];
                ch2.duration = 0;
                ch2.priority = 0;
                ch2.dataptr = None;
                self.channels[c].dataptr = backup;
                0
            }
            15 => {
                let Some(ptr) = self.get_program(v[0] as usize) else { return 0 };
                let chan = self.byte(ptr) as usize;
                if chan > 9 || self.channels[chan].dataptr.is_none() {
                    return 0;
                }
                self.channels[c].dataptr = self.channels[c].dataptr.map(|p| p - 2);
                2
            }
            16 => {
                if let Some(ins) = self.get_instrument(v[0] as usize) {
                    let ro = self.cur_reg_offset;
                    self.setup_instrument(ro, ins, c);
                }
                0
            }
            17 => {
                let ch = &mut self.channels[c];
                ch.slide_tempo = v[0];
                ch.slide_step = be16(v, 1) as i16;
                ch.primary_effect = Effect::Slide;
                ch.slide_timer = 0xFF;
                0
            }
            18 => {
                self.channels[c].primary_effect = Effect::None;
                self.channels[c].slide_step = 0;
                0
            }
            19 => {
                self.channels[c].base_freq = v[0];
                0
            }
            21 => {
                let ch = &mut self.channels[c];
                ch.vibrato_tempo = v[0];
                ch.vibrato_step_range = v[1];
                ch.vibrato_steps_countdown = v[2].wrapping_add(1);
                ch.vibrato_num_steps = v[2] << 1;
                ch.vibrato_delay = v[3];
                ch.primary_effect = Effect::Vibrato;
                0
            }
            26 => {
                self.channels[c].priority = v[0];
                0
            }
            28 => {
                self.beat_divider = v[0] >> 1;
                self.beat_div_cnt = v[0] >> 1;
                self.callback_timer = 0xFF;
                self.beat_counter = 0;
                self.beat_waiting = 0;
                0
            }
            29 => {
                if (self.beat_counter & v[0]) != 0 && self.beat_waiting != 0 {
                    self.beat_waiting = 0;
                    return 0;
                }
                if self.beat_counter & v[0] == 0 {
                    self.beat_waiting = self.beat_waiting.wrapping_add(1);
                }
                self.channels[c].dataptr = self.channels[c].dataptr.map(|p| p - 2);
                self.channels[c].duration = 1;
                2
            }
            30 => {
                self.channels[c].op_extra_level1 = v[0];
                self.adjust_volume(c);
                0
            }
            32 => {
                self.setup_duration(v[0], c);
                (v[0] != 0) as i32
            }
            33 => {
                self.setup_duration(v[0], c);
                self.note_on(c);
                (v[0] != 0) as i32
            }
            36 => {
                self.channels[c].fractional_spacing = v[0] & 7;
                0
            }
            38 => {
                self.tempo = v[0];
                0
            }
            39 => {
                self.channels[c].secondary_effect = Effect::None;
                0
            }
            41 => {
                self.channels[c].tempo = v[0];
                0
            }
            43 => {
                self.channels[c].op_extra_level3 = v[0];
                0
            }
            44 | 45 => {
                if v[0] > 9 {
                    return 0;
                }
                let backup = self.cur_channel;
                self.cur_channel = v[0] as i32;
                let c2 = v[0] as usize;
                if op == 44 {
                    self.channels[c2].op_extra_level2 = v[1];
                } else {
                    self.channels[c2].op_extra_level2 = self.channels[c2].op_extra_level2.wrapping_add(v[1]);
                }
                self.adjust_volume(c2);
                self.cur_channel = backup;
                0
            }
            46 => {
                if v[0] & 1 != 0 {
                    self.vibrato_and_am_depth_bits |= 0x80;
                } else {
                    self.vibrato_and_am_depth_bits &= 0x7F;
                }
                let b = self.vibrato_and_am_depth_bits;
                self.write_opl(0xBD, b);
                0
            }
            47 => {
                if v[0] & 1 != 0 {
                    self.vibrato_and_am_depth_bits |= 0x40;
                } else {
                    self.vibrato_and_am_depth_bits &= 0xBF;
                }
                let b = self.vibrato_and_am_depth_bits;
                self.write_opl(0xBD, b);
                0
            }
            48 => {
                self.channels[c].op_extra_level1 = self.channels[c].op_extra_level1.wrapping_add(v[0]);
                self.adjust_volume(c);
                0
            }
            51 => {
                if v[0] > 9 {
                    return 0;
                }
                let backup = self.cur_channel;
                self.cur_channel = v[0] as i32;
                let dbackup = self.channels[c].dataptr;
                let c2 = v[0] as usize;
                let ch2 = &mut self.channels[c2];
                ch2.duration = 0;
                ch2.priority = 0;
                ch2.dataptr = None;
                ch2.op_extra_level2 = 0;
                if c2 != 9 {
                    let ro = REG_OFFSET[c2];
                    self.write_opl(0xC0 + c2 as u8, 0x00);
                    self.write_opl(0x43 + ro, 0x3F);
                    self.write_opl(0x83 + ro, 0xFF);
                    self.write_opl(0xB0 + c2 as u8, 0x00);
                }
                self.cur_channel = backup;
                self.channels[c].dataptr = dbackup;
                0
            }
            53 => {
                if self.cur_channel >= 9 {
                    return 0;
                }
                let mask = be16(v, 0);
                let ch = self.channels[c];
                let mut note = (((ch.reg_bx & 0x1F) as u16) << 8) | ch.reg_ax as u16;
                let r = self.get_random_nr();
                note = note.wrapping_add(mask & r);
                note |= ((ch.reg_bx & 0x20) as u16) << 8;
                let cc = self.cur_channel as u8;
                self.write_opl(0xA0 + cc, (note & 0xFF) as u8);
                self.write_opl(0xB0 + cc, (note >> 8) as u8);
                0
            }
            54 => {
                self.channels[c].primary_effect = Effect::None;
                0
            }
            57 => {
                self.channels[c].pitch_bend = v[0] as i8;
                let rn = self.channels[c].raw_note;
                self.setup_note(rn, c, true);
                0
            }
            58 => {
                self.channels[c].tempo = self.tempo;
                0
            }
            59 | 64 => 0,
            60 => {
                self.channels[c].duration_randomness = v[0];
                0
            }
            61 => {
                self.channels[c].tempo = (self.channels[c].tempo as i32 + v[0] as i8 as i32).clamp(1, 255) as u8;
                0
            }
            63 => {
                let entry = v[1] as usize;
                if entry + 2 > 6 {
                    return 0;
                }
                if v[0] == 2 {
                    let t2 = unk_table2(entry + 1);
                    self.write_opl(0xA0, t2[0]);
                }
                0
            }
            65 => {
                let backup = self.cur_channel;
                let robackup = self.cur_reg_offset;
                for (i, ch) in [6usize, 7, 8].iter().enumerate() {
                    self.cur_channel = *ch as i32;
                    self.cur_reg_offset = REG_OFFSET[*ch];
                    if let Some(ins) = self.get_instrument(v[i] as usize) {
                        let ro = self.cur_reg_offset;
                        self.setup_instrument(ro, ins, c);
                    }
                    match ch {
                        6 => self.op_level_bd = self.channels[c].op_level2,
                        7 => {
                            self.op_level_hh = self.channels[c].op_level1;
                            self.op_level_sd = self.channels[c].op_level2;
                        }
                        _ => {
                            self.op_level_tt = self.channels[c].op_level1;
                            self.op_level_cy = self.channels[c].op_level2;
                        }
                    }
                }
                self.channels[6].reg_bx = v[3] & 0x2F;
                self.write_opl(0xB6, v[3] & 0x2F);
                self.write_opl(0xA6, v[4]);
                self.channels[7].reg_bx = v[5] & 0x2F;
                self.write_opl(0xB7, v[5] & 0x2F);
                self.write_opl(0xA7, v[6]);
                self.channels[8].reg_bx = v[7] & 0x2F;
                self.write_opl(0xB8, v[7] & 0x2F);
                self.write_opl(0xA8, v[8]);
                self.rhythm_section_bits = 0x20;
                self.cur_reg_offset = robackup;
                self.cur_channel = backup;
                0
            }
            66 => {
                let b = (self.rhythm_section_bits & !(v[0] & 0x1F)) | 0x20;
                self.write_opl(0xBD, b);
                self.rhythm_section_bits |= v[0];
                let b = self.vibrato_and_am_depth_bits | 0x20 | self.rhythm_section_bits;
                self.write_opl(0xBD, b);
                0
            }
            67 => {
                self.rhythm_section_bits = 0;
                let b = self.vibrato_and_am_depth_bits;
                self.write_opl(0xBD, b);
                0
            }
            68 => {
                let (ops, val) = (v[0], v[1] as i32);
                if ops & 1 != 0 {
                    self.op_extra_level2_hh = v[1];
                    let r = Self::check_value(val + self.op_level_hh as i32 + self.op_extra_level1_hh as i32 + self.op_extra_level2_hh as i32);
                    self.write_opl(0x51, r);
                }
                if ops & 2 != 0 {
                    self.op_extra_level2_cy = v[1];
                    let r = Self::check_value(val + self.op_level_cy as i32 + self.op_extra_level1_cy as i32 + self.op_extra_level2_cy as i32);
                    self.write_opl(0x55, r);
                }
                if ops & 4 != 0 {
                    self.op_extra_level2_tt = v[1];
                    let r = Self::check_value(val + self.op_level_tt as i32 + self.op_extra_level1_tt as i32 + self.op_extra_level2_tt as i32);
                    self.write_opl(0x52, r);
                }
                if ops & 8 != 0 {
                    self.op_extra_level2_sd = v[1];
                    let r = Self::check_value(val + self.op_level_sd as i32 + self.op_extra_level1_sd as i32 + self.op_extra_level2_sd as i32);
                    self.write_opl(0x54, r);
                }
                if ops & 16 != 0 {
                    self.op_extra_level2_bd = v[1];
                    let r = Self::check_value(val + self.op_level_bd as i32 + self.op_extra_level1_bd as i32 + self.op_extra_level2_bd as i32);
                    self.write_opl(0x53, r);
                }
                0
            }
            69 => {
                let (ops, val) = (v[0], v[1] as i32);
                if ops & 1 != 0 {
                    self.op_extra_level1_hh = Self::check_value(val + self.op_level_hh as i32 + self.op_extra_level1_hh as i32 + self.op_extra_level2_hh as i32);
                    let r = self.op_extra_level1_hh;
                    self.write_opl(0x51, r);
                }
                if ops & 2 != 0 {
                    self.op_extra_level1_cy = Self::check_value(val + self.op_level_cy as i32 + self.op_extra_level1_cy as i32 + self.op_extra_level2_cy as i32);
                    let r = self.op_extra_level1_cy;
                    self.write_opl(0x55, r);
                }
                if ops & 4 != 0 {
                    self.op_extra_level1_tt = Self::check_value(val + self.op_level_tt as i32 + self.op_extra_level1_tt as i32 + self.op_extra_level2_tt as i32);
                    let r = self.op_extra_level1_tt;
                    self.write_opl(0x52, r);
                }
                if ops & 8 != 0 {
                    self.op_extra_level1_sd = Self::check_value(val + self.op_level_sd as i32 + self.op_extra_level1_sd as i32 + self.op_extra_level2_sd as i32);
                    let r = self.op_extra_level1_sd;
                    self.write_opl(0x54, r);
                }
                if ops & 16 != 0 {
                    self.op_extra_level1_bd = Self::check_value(val + self.op_level_bd as i32 + self.op_extra_level1_bd as i32 + self.op_extra_level2_bd as i32);
                    let r = self.op_extra_level1_bd;
                    self.write_opl(0x53, r);
                }
                0
            }
            70 => {
                let (ops, val) = (v[0], v[1] as i32);
                if ops & 1 != 0 {
                    self.op_extra_level1_hh = v[1];
                    let r = Self::check_value(val + self.op_level_hh as i32 + self.op_extra_level2_hh as i32);
                    self.write_opl(0x51, r);
                }
                if ops & 2 != 0 {
                    self.op_extra_level1_cy = v[1];
                    let r = Self::check_value(val + self.op_level_cy as i32 + self.op_extra_level2_cy as i32);
                    self.write_opl(0x55, r);
                }
                if ops & 4 != 0 {
                    self.op_extra_level1_tt = v[1];
                    let r = Self::check_value(val + self.op_level_tt as i32 + self.op_extra_level2_tt as i32);
                    self.write_opl(0x52, r);
                }
                if ops & 8 != 0 {
                    self.op_extra_level1_sd = v[1];
                    let r = Self::check_value(val + self.op_level_sd as i32 + self.op_extra_level2_sd as i32);
                    self.write_opl(0x54, r);
                }
                if ops & 16 != 0 {
                    self.op_extra_level1_bd = v[1];
                    let r = Self::check_value(val + self.op_level_bd as i32 + self.op_extra_level2_bd as i32);
                    self.write_opl(0x53, r);
                }
                0
            }
            71 => {
                self.sound_trigger = v[0];
                0
            }
            72 => {
                self.channels[c].tempo_reset = v[0];
                0
            }
            73 => 0,
            _ => self.update_stop_channel(c),
        }
    }
}
