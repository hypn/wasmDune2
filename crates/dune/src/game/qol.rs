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

use super::*;

/// How close (in tiles) to its last field a harvester counts as being in it, and how far around
/// that tile spice still counts as the same field.
const FIELD_RADIUS: u16 = 4;

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
}
