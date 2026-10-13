//! Explosions (explosion.c).

use super::map::*;
use super::*;
use crate::tables::landscape::LANDSCAPE_INFO;
use crate::tables::EXPLOSIONS;

pub const EXPLOSION_MAX: usize = 32;

const EXPLOSION_STOP: u8 = 0;
const EXPLOSION_SET_SPRITE: u8 = 1;
const EXPLOSION_SET_TIMEOUT: u8 = 2;
const EXPLOSION_SET_RANDOM_TIMEOUT: u8 = 3;
const EXPLOSION_MOVE_Y_POSITION: u8 = 4;
const EXPLOSION_TILE_DAMAGE: u8 = 5;
const EXPLOSION_PLAY_VOICE: u8 = 6;
const EXPLOSION_SCREEN_SHAKE: u8 = 7;
const EXPLOSION_SET_ANIMATION: u8 = 8;
const EXPLOSION_BLOOM_EXPLOSION: u8 = 9;

impl Game {
    pub fn explosion_init(&mut self) {
        self.explosions = vec![
            Explosion { time_out: 0, house_id: 0, is_dirty: false, current: 0, sprite_id: 0, commands: None, position: Tile32::default() };
            EXPLOSION_MAX
        ];
    }

    fn explosion_func_tile_damage(&mut self, i: usize) {
        const CRATER_ICON_MAP_INDEX: [i16; 3] = [-1, 2, 1];
        let e = self.explosions[i];
        let packed = e.position.pack();
        if !self.map_is_position_unveiled(packed) {
            return;
        }
        let type_ = self.map_get_landscape_type(packed);
        if type_ == LST_STRUCTURE || type_ == LST_DESTROYED_WALL {
            return;
        }
        let p = packed as usize;
        if type_ == LST_CONCRETE_SLAB {
            // groundTileID is a 9-bit field in the original: the 0x8000 "built on" flag of
            // g_mapTileID falls off (kept, it is an invalid tile and draws black).
            self.map[p].ground_tile_id = self.map_tile_id[p] & 0x1FF;
        }
        let crater = LANDSCAPE_INFO[type_ as usize].crater_type;
        if crater == 0 {
            return;
        }
        let mut overlay = self.map[p].overlay_tile_id;
        if !self.tile_is_unveiled(overlay) {
            return;
        }
        let group = CRATER_ICON_MAP_INDEX[crater as usize] as usize;
        let base = self.icon_map[group] as usize;
        let first = self.icon_map[base];
        let last = self.icon_map[base + 10];
        if first <= overlay && overlay <= last {
            overlay -= first;
            if overlay < 4 {
                overlay += 2;
            }
        } else {
            overlay = (self.rng.random_256() & 1) as u16;
        }
        self.map_change_spice_amount(packed, -1);
        if self.map[p].ground_tile_id == self.bloom_tile_id {
            let ph = self.player_house_id;
            self.map_bloom_explode_spice(packed, ph);
            return;
        }
        self.map[p].overlay_tile_id = overlay + first;
    }

    fn explosion_func_set_animation(&mut self, i: usize, animation_map_id: u16) {
        let e = self.explosions[i];
        let packed = e.position.pack();
        if self.structure_get_by_packed_tile(packed).is_some() {
            return;
        }
        let mut id = animation_map_id + (self.rng.random_256() & 1) as u16;
        id += if LANDSCAPE_INFO[self.map_get_landscape_type(packed) as usize].is_sand { 0 } else { 2 };
        self.animation_start(Some((AnimTable::Map, id)), e.position, 0, e.house_id, 3);
    }

    fn explosion_func_stop(&mut self, i: usize) {
        let p = self.explosions[i].position.pack();
        self.map[p as usize].has_explosion = false;
        self.explosions[i].commands = None;
    }

    fn explosion_stop_at_position(&mut self, packed: u16) {
        if !self.map[packed as usize].has_explosion {
            return;
        }
        for i in 0..EXPLOSION_MAX {
            let e = self.explosions[i];
            if e.commands.is_none() || e.position.pack() != packed {
                continue;
            }
            self.explosion_func_stop(i);
        }
    }

    /// Explosion_Start
    pub fn explosion_start(&mut self, explosion_type: u16, position: Tile32) {
        if explosion_type > EXPLOSION_SPICE_BLOOM_TREMOR {
            return;
        }
        let packed = position.pack();
        self.explosion_stop_at_position(packed);
        for i in 0..EXPLOSION_MAX {
            if self.explosions[i].commands.is_some() {
                continue;
            }
            self.explosions[i] = Explosion {
                time_out: self.timer_gui,
                house_id: self.explosions[i].house_id,
                is_dirty: false,
                current: 0,
                sprite_id: 0,
                commands: Some(explosion_type),
                position,
            };
            self.ticks.explosion_timer = 0;
            self.map[packed as usize].has_explosion = true;
            break;
        }
    }

    /// Explosion_Tick
    pub fn explosion_tick(&mut self) {
        if self.ticks.explosion_timer > self.timer_gui {
            return;
        }
        self.ticks.explosion_timer = self.ticks.explosion_timer.wrapping_add(10000);
        for i in 0..EXPLOSION_MAX {
            let Some(cmds) = self.explosions[i].commands else { continue };
            if self.explosions[i].time_out <= self.timer_gui {
                let list = EXPLOSIONS[cmds as usize];
                let cur = self.explosions[i].current as usize;
                let cmd = list.get(cur).copied().unwrap_or(crate::tables::ExplosionCmd(0, 0));
                self.explosions[i].current += 1;
                let parameter = cmd.1;
                match cmd.0 {
                    EXPLOSION_SET_SPRITE => self.explosions[i].sprite_id = parameter,
                    EXPLOSION_SET_TIMEOUT => self.explosions[i].time_out = self.timer_gui + parameter as u32,
                    EXPLOSION_SET_RANDOM_TIMEOUT => {
                        self.explosions[i].time_out = self.timer_gui + self.rng.lcg_range(0, parameter) as u32;
                    }
                    EXPLOSION_MOVE_Y_POSITION => {
                        let e = &mut self.explosions[i];
                        e.position.y = e.position.y.wrapping_add(parameter);
                    }
                    EXPLOSION_TILE_DAMAGE => self.explosion_func_tile_damage(i),
                    EXPLOSION_PLAY_VOICE => {
                        let p = self.explosions[i].position;
                        self.voice_at_tile(parameter, p);
                    }
                    EXPLOSION_SCREEN_SHAKE => self.emit(Event::ScreenShake),
                    EXPLOSION_SET_ANIMATION => self.explosion_func_set_animation(i, parameter),
                    EXPLOSION_BLOOM_EXPLOSION => {
                        let packed = self.explosions[i].position.pack();
                        if self.map[packed as usize].ground_tile_id == self.bloom_tile_id {
                            let ph = self.player_house_id;
                            self.map_bloom_explode_spice(packed, ph);
                        }
                    }
                    EXPLOSION_STOP | _ => self.explosion_func_stop(i),
                }
            }
            let e = self.explosions[i];
            if e.commands.is_none() || e.time_out > self.ticks.explosion_timer {
                continue;
            }
            self.ticks.explosion_timer = e.time_out;
        }
    }
}
