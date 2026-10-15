//! Simulation-side quality-of-life options (NOT part of the original). Each is off by default,
//! which keeps the original behaviour.
//!
//! `GameConfig::harvester_return`: the original's harvester only remembers where it was
//! harvesting (Unit.targetLast) when a carryall takes it to the refinery; driving there itself
//! clears it, and leaving the refinery it takes whatever spice Map_SearchSpice finds nearest the
//! refinery, seen or not. With the option on, the player's harvesters head back to the tile they
//! last harvested while spice the player has seen is left around it, else to the nearest spice
//! the player has seen, and only when none has been seen anywhere to what the original finds. A
//! carryall may also fly them back. Computer harvesters keep the original behaviour.
//!
//! `GameConfig::carryall_timeout`: in the original, a carryall holding one of the player's units
//! that has no destination (a full harvester with no free refinery, or an empty one with nowhere
//! to go) circles, retrying every second or so, for as long as it takes. With the option on, one
//! that has done so for `CARRYALL_TIMEOUT` seconds is given a spot to set the unit down: beside
//! the nearest refinery for a full harvester, the spice an empty one would look for, else a free
//! tile close by. The script then flies there and delivers as it would any tile destination.

use super::map::LST_BLOOM_FIELD;
use super::*;
use crate::tile::{pack_xy, packed_x, packed_y};

/// How close (in tiles) to its last field a harvester counts as being in it, and how far around
/// that tile spice still counts as the same field.
const FIELD_RADIUS: u16 = 4;

/// Seconds (60 game ticks each) a carryall may hold a unit with no destination.
const CARRYALL_TIMEOUT: u8 = 20;

impl Game {
    /// Whether `u` is one of the player's harvesters with `harvester_return` on.
    pub fn qol_harvester(&self, u: u16) -> bool {
        self.config.harvester_return && self.u(u).o.type_ == UNIT_HARVESTER && self.unit_get_house_id(u) == self.player_house_id
    }

    /// Where a player's harvester looks for spice from `from` (Map_SearchSpice's `radius`).
    pub fn qol_harvester_spice(&mut self, u: u16, from: u16, radius: u16) -> u16 {
        let last = self.u(u).last_harvest;
        if last != 0 {
            if get_distance_packed(from, last) <= FIELD_RADIUS {
                // In the field: carry on as the original, over spice that has been seen.
                let p = self.map_search_spice_where(from, radius, true);
                if p != 0 {
                    return p;
                }
            }
            let p = self.map_search_spice_where(last, FIELD_RADIUS, true);
            if p != 0 {
                return p;
            }
            // That field is used up (or occupied).
            self.um(u).last_harvest = 0;
        }
        let p = self.map_search_spice_where(from, 64, true);
        if p != 0 {
            return p;
        }
        self.map_search_spice(from, radius)
    }

    /// A player's harvester has just left the refinery (which puts it beside the spice nearest
    /// the refinery): if its field is elsewhere, send it there, as the "Harvest" order does.
    pub fn qol_harvester_leave(&mut self, u: u16) {
        if !self.qol_harvester(u) {
            return;
        }
        let from = self.u(u).o.position.pack();
        let last = self.u(u).last_harvest;
        if last == 0 || get_distance_packed(from, last) <= FIELD_RADIUS {
            return;
        }
        let target = self.qol_harvester_spice(u, from, 20);
        if target == 0 || get_distance_packed(from, target) <= FIELD_RADIUS {
            return;
        }
        // As the player's "Harvest" order (GUI_Widget_Viewport_Click).
        self.object_script_variable4_clear(ObjRef::Unit(u));
        {
            let unit = self.um(u);
            unit.target_attack = 0;
            unit.target_move = 0;
            unit.route[0] = 0xFF;
        }
        let e = self.tile_encode(target);
        self.unit_set_action(u, ACTION_HARVEST);
        self.um(u).target_move = e;
    }

    /// Whether a carryall should take this harvester from the refinery at `from` back to its
    /// field (the original only does that when one brought it in: Unit.targetLast is set): when
    /// the field is more than a short drive away.
    pub fn qol_harvester_wants_lift(&self, u: u16, from: u16) -> bool {
        let last = self.u(u).last_harvest;
        self.qol_harvester(u) && last != 0 && get_distance_packed(from, last) > 2 * FIELD_RADIUS
    }

    /// Once a second: give the player's carryalls that have been stuck holding a unit a destination.
    pub fn qol_carryall_tick(&mut self) {
        if !self.config.carryall_timeout || self.timer_game % 60 != 0 {
            return;
        }
        let h = self.player_house_id;
        for c in self.units_of(h, UNIT_CARRYALL as u16) {
            let un = self.u(c);
            let stuck = un.o.linked_id != 0xFF && un.o.flags.in_transport && un.target_move == 0;
            if !stuck {
                let unit = self.um(c);
                unit.carry_idle = 0;
                if unit.o.linked_id == 0xFF {
                    unit.carry_drop = 0;
                }
                continue;
            }
            self.um(c).carry_idle += 1;
            if self.u(c).carry_idle < CARRYALL_TIMEOUT {
                continue;
            }
            self.um(c).carry_idle = 0;
            let l = self.u(c).o.linked_id as u16;
            let here = self.u(c).o.position.pack();
            let mut centre = here;
            if self.u(l).o.type_ == UNIT_HARVESTER {
                if self.u(l).amount != 0 {
                    // Full: set it down by the nearest refinery; it drives in when one is free.
                    let pos = self.u(c).o.position;
                    if let Some(r) = self.structures_of(h, STRUCTURE_REFINERY as u16).into_iter().min_by_key(|&r| get_distance(pos, self.s(r).o.position)) {
                        centre = self.s(r).o.position.pack();
                    }
                } else if self.qol_harvester(l) {
                    let spice = self.qol_harvester_spice(l, here, 20);
                    if spice != 0 {
                        centre = spice;
                    }
                }
            }
            if let Some(p) = self.qol_free_ground_near(centre, l) {
                // Set directly for a carryall that is just meandering, and via su_move_to_structure
                // for one that is searching for a refinery (its script resets target_move after).
                let e = self.tile_encode(p);
                let unit = self.um(c);
                unit.target_move = e;
                unit.carry_drop = e;
            }
        }
    }

    /// The closest free tile (within 6) around `centre` that unit `u` can stand on.
    fn qol_free_ground_near(&self, centre: u16, u: u16) -> Option<u16> {
        let mt = self.ui(u).movement_type as usize;
        let (cx, cy) = (packed_x(centre) as i32, packed_y(centre) as i32);
        let mut best: Option<(u16, u16)> = None;
        for dy in -6i32..=6 {
            for dx in -6i32..=6 {
                let (x, y) = (cx + dx, cy + dy);
                if !(0..64).contains(&x) || !(0..64).contains(&y) {
                    continue;
                }
                let p = pack_xy(x as u16, y as u16);
                if !self.map_is_valid_position(p) || self.object_get_by_packed_tile(p).is_some() {
                    continue;
                }
                if self.map_get_landscape_type(p) == LST_BLOOM_FIELD || self.landscape_info(p).movement_speed[mt] == 0 {
                    continue;
                }
                let d = get_distance_packed(centre, p);
                if best.map_or(true, |(_, bd)| d < bd) {
                    best = Some((p, d));
                }
            }
        }
        best.map(|b| b.0)
    }
}
