//! Tile animations (animation.c): structure lights, sand tracks, explosion craters, ...

use super::*;
use crate::tables::{AnimationCmd, ANIMATION_MAP, ANIMATION_STRUCTURE, ANIMATION_UNIT_MOVE, ANIMATION_UNIT_SCRIPT1, ANIMATION_UNIT_SCRIPT2, STRUCTURE_LAYOUT_TILES, STRUCTURE_LAYOUT_TILE_COUNT};

pub const ANIMATION_MAX: usize = 112;

const ICM_ICONGROUP_BASE_DEFENSE_TURRET: u8 = 23;
const ICM_ICONGROUP_BASE_ROCKET_TURRET: u8 = 24;

/// Read a command, allowing reads past the end of a row into the following rows
/// (the original walks raw pointers through the 2D tables).
fn command(table: AnimTable, row: u16, index: usize) -> AnimationCmd {
    fn flat<const W: usize, const H: usize>(t: &[[AnimationCmd; W]; H], row: usize, index: usize) -> AnimationCmd {
        let i = row * W + index;
        t.get(i / W).map(|r| r[i % W]).unwrap_or(AnimationCmd(0, 0))
    }
    let row = row as usize;
    match table {
        AnimTable::UnitMove => flat(&ANIMATION_UNIT_MOVE, row, index),
        AnimTable::UnitScript1 => flat(&ANIMATION_UNIT_SCRIPT1, row, index),
        AnimTable::UnitScript2 => flat(&ANIMATION_UNIT_SCRIPT2, row, index),
        AnimTable::Map => flat(&ANIMATION_MAP, row, index),
        AnimTable::Structure => flat(&ANIMATION_STRUCTURE, row, index),
    }
}

impl Game {
    pub fn animation_init(&mut self) {
        self.animations = vec![
            Animation { tick_next: 0, tile_layout: 0, house_id: 0, current: 0, icon_group: 0, commands: None, tile: Tile32::default() };
            ANIMATION_MAX
        ];
    }

    fn animation_func_stop(&mut self, i: usize) {
        let a = self.animations[i];
        let layout = a.tile_layout as usize;
        let packed = a.tile.pack();
        self.map[packed as usize].has_animation = false;
        self.animations[i].commands = None;
        for j in 0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
            let position = (packed.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]) & 0xFFF) as usize;
            if a.tile_layout != 0 {
                self.map[position].ground_tile_id = self.map_tile_id[position];
            }
            if self.map_is_position_unveiled(position as u16) {
                self.map[position].overlay_tile_id = 0;
            }
        }
    }

    fn animation_func_set_overlay_tile(&mut self, i: usize, parameter: i16) {
        let a = self.animations[i];
        let packed = a.tile.pack();
        if !self.map_is_position_unveiled(packed) {
            return;
        }
        let id = self.icon_map[(self.icon_map[a.icon_group as usize] as i32 + parameter as i32) as usize];
        let t = &mut self.map[packed as usize];
        t.overlay_tile_id = id;
        t.house_id = a.house_id;
    }

    fn animation_func_set_ground_tile(&mut self, i: usize, parameter: i16) {
        let a = self.animations[i];
        let layout = a.tile_layout as usize;
        let count = STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize;
        let packed = a.tile.pack();
        let base = (self.icon_map[a.icon_group as usize] as i32 + count as i32 * parameter as i32) as usize;
        let special = if parameter > 1 && (a.icon_group == ICM_ICONGROUP_BASE_DEFENSE_TURRET || a.icon_group == ICM_ICONGROUP_BASE_ROCKET_TURRET) {
            self.structure_get_by_packed_tile(packed).map(|s| {
                self.s(s).rotation_sprite_diff + self.icon_map[self.icon_map[a.icon_group as usize] as usize] + 2
            })
        } else {
            None
        };
        for j in 0..count {
            let position = (packed.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]) & 0xFFF) as usize;
            let tile_id = match special {
                Some(t) if j == 0 => t,
                Some(_) => self.icon_map.get(base + j).copied().unwrap_or(0),
                None => self.icon_map.get(base + j).copied().unwrap_or(0),
            };
            if self.map[position].ground_tile_id == tile_id {
                continue;
            }
            self.map[position].ground_tile_id = tile_id;
            self.map[position].house_id = a.house_id;
            if self.map_is_position_unveiled(position as u16) {
                self.map[position].overlay_tile_id = 0;
            }
        }
    }

    /// Animation_Start
    pub fn animation_start(&mut self, commands: Option<(AnimTable, u16)>, tile: Tile32, tile_layout: u16, house_id: u8, icon_group: u8) {
        let packed = tile.pack();
        self.animation_stop_by_tile(packed);
        for i in 0..ANIMATION_MAX {
            if self.animations[i].commands.is_some() {
                continue;
            }
            self.animations[i] = Animation { tick_next: self.timer_gui, tile_layout, house_id, current: 0, icon_group, commands, tile };
            self.ticks.animation_timer = 0;
            let t = &mut self.map[packed as usize];
            t.house_id = house_id;
            t.has_animation = true;
            return;
        }
    }

    /// Animation_Stop_ByTile
    pub fn animation_stop_by_tile(&mut self, packed: u16) {
        if !self.map[(packed & 0xFFF) as usize].has_animation {
            return;
        }
        for i in 0..ANIMATION_MAX {
            let a = self.animations[i];
            if a.commands.is_none() || a.tile.pack() != packed {
                continue;
            }
            self.animation_func_stop(i);
            return;
        }
    }

    /// Animation_Tick
    pub fn animation_tick(&mut self) {
        if self.ticks.animation_timer > self.timer_gui {
            return;
        }
        self.ticks.animation_timer = self.ticks.animation_timer.wrapping_add(10000);
        for i in 0..ANIMATION_MAX {
            let Some((table, row)) = self.animations[i].commands else { continue };
            if self.animations[i].tick_next <= self.timer_gui {
                let cmd = command(table, row, self.animations[i].current as usize);
                let parameter = cmd.1 as i16;
                self.animations[i].current = self.animations[i].current.wrapping_add(1);
                match cmd.0 {
                    1 => {
                        // ABORT
                        let p = self.animations[i].tile.pack();
                        self.map[p as usize].has_animation = false;
                        self.animations[i].commands = None;
                    }
                    2 => self.animation_func_set_overlay_tile(i, parameter),
                    3 => {
                        let r = (self.rng.random_256() % 4) as u32;
                        self.animations[i].tick_next = self.timer_gui.wrapping_add(parameter as i32 as u32).wrapping_add(r);
                    }
                    4 => self.animations[i].current = 0,
                    5 => {
                        let t = self.animations[i].tile;
                        self.voice_at_tile(parameter as u16, t);
                    }
                    6 => self.animation_func_set_ground_tile(i, parameter),
                    7 => {
                        let a = &mut self.animations[i];
                        a.current = a.current.wrapping_add((parameter - 1) as u8);
                    }
                    8 => self.animations[i].icon_group = parameter as u8,
                    _ => self.animation_func_stop(i),
                }
                if self.animations[i].commands.is_none() {
                    continue;
                }
            }
            if self.animations[i].tick_next < self.ticks.animation_timer {
                self.ticks.animation_timer = self.animations[i].tick_next;
            }
        }
    }
}
