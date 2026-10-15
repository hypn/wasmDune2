//! Structures (structure.c).

use super::map::*;
use super::script::Ctx;
use super::*;
use crate::data::image::ICM_ICONGROUP_LANDSCAPE;
use crate::tables::landscape::LANDSCAPE_INFO;
use crate::tables::strings::*;
use crate::tables::{HOUSE_INFO, MAP_DIFF, STRUCTURE_LAYOUT_TILES, STRUCTURE_LAYOUT_TILES_AROUND, STRUCTURE_LAYOUT_TILE_COUNT, STRUCTURE_LAYOUT_TILE_DIFF};

const ICM_ICONGROUP_BASE_DEFENSE_TURRET: usize = 23;
const ICM_ICONGROUP_BASE_ROCKET_TURRET: usize = 24;

/// What the player picked in a factory window (GUI_DisplayFactoryWindow result).
#[derive(Clone, Debug)]
pub enum FactoryResult {
    Resume,
    Upgrade,
    /// (objectType, amount) pairs, in window order.
    Buy(Vec<(u16, u16)>),
}

fn is_slab_or_wall(t: u8) -> bool {
    t == STRUCTURE_SLAB_1X1 || t == STRUCTURE_SLAB_2X2 || t == STRUCTURE_WALL
}

impl Game {
    /// GameLoop_Structure
    pub fn game_loop_structure(&mut self) {
        let now = self.timer_game;
        let mut tick_degrade = false;
        let mut tick_structure = false;
        let mut tick_script = false;
        let mut tick_palace = false;
        if self.ticks.structure_degrade <= now && self.campaign_id > 1 {
            tick_degrade = true;
            // Not the original's Tools_AdjustToGameSpeed(10800, 5400, 21600, inverse), which decays
            // twice as often at Fastest: 10800 ticks is 180 real seconds at 60 Hz whatever the game
            // speed, stretched by Turbo (which runs that many ticks faster).
            self.ticks.structure_degrade = now + 10800 * self.config.sim_percent.max(10) / 100;
        }
        if self.ticks.structure_structure <= now {
            tick_structure = true;
            self.ticks.structure_structure = now + self.adjust_to_game_speed(30, 15, 60, true) as u32;
        }
        if self.ticks.structure_script <= now {
            tick_script = true;
            self.ticks.structure_script = now + 5;
        }
        if self.ticks.structure_palace <= now {
            tick_palace = true;
            self.ticks.structure_palace = now + 60;
        }

        let mut find = PoolFind::all();
        while let Some(s) = self.structure_find(&mut find) {
            if is_slab_or_wall(self.s(s).o.type_) {
                continue;
            }
            let si = *self.si(s);
            let hid = self.s(s).o.house_id;
            let hi = HOUSE_INFO[hid as usize];
            self.script_current_object = Some(ObjRef::Structure(s));
            self.script_current_structure = Some(s);
            self.script_current_unit = None;
            self.script_current_team = None;

            if tick_palace && self.s(s).o.type_ == STRUCTURE_PALACE {
                if self.s(s).count_down != 0 {
                    self.sm(s).count_down -= 1;
                }
                if self.s(s).count_down == 0 && !self.h(hid).flags.human && self.h(hid).flags.is_ai_active {
                    self.structure_activate_special(s);
                }
            }

            if tick_degrade && self.s(s).o.flags.degrades && self.s(s).o.hitpoints > si.o.hitpoints / 2 {
                self.structure_damage(s, hi.degrading_amount, 0);
            }

            if tick_structure {
                let was_upgrading = self.s(s).o.flags.upgrading;
                if was_upgrading {
                    let upgrade_cost = si.o.build_credits / 40;
                    if upgrade_cost <= self.h(hid).credits {
                        self.hm(hid).credits -= upgrade_cost;
                        if self.s(s).upgrade_time_left > 5 {
                            self.sm(s).upgrade_time_left -= 5;
                        } else {
                            self.sm(s).upgrade_level += 1;
                            self.sm(s).o.flags.upgrading = false;
                            if hid == HOUSE_ORDOS && self.s(s).o.type_ == STRUCTURE_HEAVY_VEHICLE && self.s(s).upgrade_level == 2 {
                                self.sm(s).upgrade_level = 3;
                            }
                            let up = self.structure_is_upgradable(s);
                            self.sm(s).upgrade_time_left = if up { 100 } else { 0 };
                        }
                    } else {
                        self.sm(s).o.flags.upgrading = false;
                    }
                }
                // QoL (repair_while_upgrading): a repair also goes on during an upgrade.
                if self.s(s).o.flags.repairing && (!was_upgrading || self.config.repair_while_upgrading) {
                    let repair_cost = if self.config.dune2_enhanced {
                        (si.o.build_credits as u32 * 2 / si.o.hitpoints as u32) as u16
                    } else {
                        (((2 * 256 / si.o.hitpoints as u32) * si.o.build_credits as u32 + 128) / 256) as u16
                    };
                    if repair_cost <= self.h(hid).credits {
                        self.hm(hid).credits -= repair_cost;
                        let inc = if hid == self.player_house_id || self.campaign_id >= 3 { 5 } else { 3 };
                        self.sm(s).o.hitpoints += inc;
                        if self.s(s).o.hitpoints > si.o.hitpoints {
                            let st = self.sm(s);
                            st.o.hitpoints = si.o.hitpoints;
                            st.o.flags.repairing = false;
                            st.o.flags.on_hold = false;
                        }
                    } else {
                        self.sm(s).o.flags.repairing = false;
                    }
                } else if !was_upgrading {
                    self.structure_tick_build(s, &si, hid);
                    if self.s(s).o.type_ == STRUCTURE_REPAIR {
                        let st = self.s(s);
                        if !st.o.flags.on_hold && st.count_down != 0 && st.o.linked_id != 0xFF {
                            let lu = st.o.linked_id as u16;
                            let ui_credits = self.ui(lu).o.build_credits;
                            let mut repair_speed: u16 = 256;
                            if self.s(s).o.hitpoints < si.o.hitpoints {
                                repair_speed = (self.s(s).o.hitpoints as u32 * 256 / si.o.hitpoints as u32) as u16;
                            }
                            let repair_cost = (2 * ui_credits as u32 / 256) as u16;
                            if repair_cost < self.h(hid).credits {
                                self.hm(hid).credits -= repair_cost;
                                if repair_speed < self.s(s).count_down {
                                    self.sm(s).count_down -= repair_speed;
                                } else {
                                    self.sm(s).count_down = 0;
                                    self.structure_set_state(s, STRUCTURE_STATE_READY);
                                    if hid == self.player_house_id {
                                        self.feedback(self.player_house_id as u16 + 55);
                                    }
                                }
                            }
                        } else if self.h(hid).credits != 0 {
                            self.sm(s).o.flags.on_hold = false;
                        }
                    }

                    // AI maintenance on structures
                    if self.h(hid).flags.is_ai_active && self.s(s).o.flags.allocated && hid != self.player_house_id && self.h(hid).credits != 0 {
                        if self.s(s).o.hitpoints < si.o.hitpoints / 2 {
                            self.structure_set_repairing_state(s, 1);
                        }
                        if si.o.flags.factory && self.s(s).count_down == 0 && self.s(s).o.linked_id == 0xFF {
                            let t = self.structure_ai_pick_next_to_build(s);
                            if t != 0xFFFF {
                                self.structure_build_object(s, t);
                            }
                        }
                    }
                }
            }

            if tick_script {
                if self.s(s).o.script.delay != 0 {
                    self.sm(s).o.script.delay -= 1;
                } else if self.s(s).o.script.is_loaded() {
                    for _ in 0..3 {
                        if !self.script_run(Ctx::Structure(s)) {
                            break;
                        }
                    }
                } else {
                    let t = self.s(s).o.type_;
                    self.sm(s).o.script.reset(ScriptKind::Structure);
                    self.script_load(Ctx::Structure(s), t);
                }
            }
        }
    }

    /// The factory part of the GameLoop_Structure tick.
    fn structure_tick_build(&mut self, s: u16, si: &StructureInfo, hid: u8) {
        let st = self.s(s);
        if st.o.flags.on_hold || st.count_down == 0 || st.o.linked_id == 0xFF || st.state != STRUCTURE_STATE_BUSY || !si.o.flags.factory {
            return;
        }
        let oi = if st.o.type_ == STRUCTURE_CONSTRUCTION_YARD {
            self.structure_info[st.object_type as usize].o
        } else if st.o.type_ == STRUCTURE_REPAIR {
            self.ui(st.o.linked_id as u16).o
        } else {
            self.unit_info[st.object_type as usize].o
        };
        let mut build_speed: u16 = 256;
        if st.o.hitpoints < si.o.hitpoints {
            build_speed = (st.o.hitpoints as u32 * 256 / si.o.hitpoints as u32) as u16;
        }
        if self.player_house_id != hid {
            let campaign = if self.skirmish.enabled { self.skirmish.ai_speed_campaign } else { self.campaign_id };
            let cap = campaign * 20 + 95;
            if build_speed > cap {
                build_speed = cap;
            }
        }
        let mut build_cost = (oi.build_credits as u32 * 256 / oi.build_time as u32) as u16;
        if build_speed < 256 {
            build_cost = (build_speed as u32 * build_cost as u32 / 256) as u16;
        }
        if st.o.type_ == STRUCTURE_REPAIR && build_cost > 4 {
            build_cost /= 4;
        }
        let build_cost = build_cost.wrapping_add(st.build_cost_remainder);
        if build_cost / 256 <= self.h(hid).credits {
            self.sm(s).build_cost_remainder = build_cost & 0xFF;
            self.hm(hid).credits -= build_cost / 256;
            if build_speed < self.s(s).count_down {
                self.sm(s).count_down -= build_speed;
            } else {
                self.sm(s).count_down = 0;
                self.sm(s).build_cost_remainder = 0;
                self.structure_set_state(s, STRUCTURE_STATE_READY);
                let t = self.s(s).o.type_;
                if hid == self.player_house_id {
                    if t != STRUCTURE_BARRACKS && t != STRUCTURE_WOR_TROOPER {
                        let mut string_id = STR_IS_COMPLETED_AND_AWAITING_ORDERS;
                        if t == STRUCTURE_HIGH_TECH {
                            string_id = STR_IS_COMPLETE;
                        }
                        if t == STRUCTURE_CONSTRUCTION_YARD {
                            string_id = STR_IS_COMPLETED_AND_READY_TO_PLACE;
                        }
                        let mut text = self.string(oi.string_id_full);
                        text.push(b' ');
                        text.extend(self.string(string_id));
                        self.display_text(text, 0);
                        self.feedback(0);
                    }
                } else if t == STRUCTURE_CONSTRUCTION_YARD {
                    // An AI immediately places the structure when it is done building.
                    let ns = self.s(s).o.linked_id as u16;
                    self.sm(s).o.linked_id = 0xFF;
                    self.structure_set_state(s, STRUCTURE_STATE_IDLE);
                    let mut placed = false;
                    self.skirmish_drop_blocked_rebuilds(hid, false);
                    for i in 0..5 {
                        let reb = self.h(hid).ai_structure_rebuild[i];
                        if self.s(ns).o.type_ as u16 != reb[0] {
                            continue;
                        }
                        if !self.structure_place(ns, reb[1]) {
                            continue;
                        }
                        self.hm(hid).ai_structure_rebuild[i] = [0, 0];
                        placed = true;
                        break;
                    }
                    if !placed && !self.skirmish_ai_place_structure(hid, ns) {
                        let credits = self.si(ns).o.build_credits;
                        self.hm(hid).credits = self.h(hid).credits.wrapping_add(credits);
                        self.structure_free(ns);
                    }
                }
            }
        } else if hid == self.player_house_id {
            self.sm(s).o.flags.on_hold = true;
            self.display_string(STR_INSUFFICIENT_FUNDS_CONSTRUCTION_IS_HALTED, 0);
        }
    }

    /// Structure_Create
    pub fn structure_create(&mut self, index: u16, type_id: u8, house_id: u8, position: u16) -> Option<u16> {
        if house_id >= HOUSE_MAX || type_id as usize >= STRUCTURE_MAX {
            return None;
        }
        let si = self.structure_info[type_id as usize];
        let s = self.structure_allocate(index, type_id)?;
        {
            let st = self.sm(s);
            st.o.house_id = house_id;
            st.creator_house_id = house_id as u16;
            st.o.flags.is_not_on_map = true;
            st.o.position = Tile32::default();
            st.o.linked_id = 0xFF;
            st.state = STRUCTURE_STATE_JUSTBUILT;
        }
        if type_id == STRUCTURE_TURRET {
            self.sm(s).rotation_sprite_diff = self.icon_map[self.icon_map[ICM_ICONGROUP_BASE_DEFENSE_TURRET] as usize + 1];
        }
        if type_id == STRUCTURE_ROCKET_TURRET {
            self.sm(s).rotation_sprite_diff = self.icon_map[self.icon_map[ICM_ICONGROUP_BASE_ROCKET_TURRET] as usize + 1];
        }
        self.sm(s).o.hitpoints = si.o.hitpoints;
        self.sm(s).hitpoints_max = si.o.hitpoints;
        if house_id == HOUSE_HARKONNEN && type_id == STRUCTURE_LIGHT_VEHICLE {
            self.sm(s).upgrade_level = 1;
        }
        if si.o.flags.factory {
            let up = self.structure_is_upgradable(s);
            self.sm(s).upgrade_time_left = if up { 100 } else { 0 };
        }
        self.sm(s).object_type = 0xFFFF;
        self.structure_build_object(s, 0xFFFE);
        self.sm(s).count_down = 0;
        if house_id != self.player_house_id {
            while self.structure_is_upgradable(s) {
                self.sm(s).upgrade_level += 1;
            }
            self.sm(s).upgrade_time_left = 0;
        }
        if position != 0xFFFF && !self.structure_place(s, position) {
            self.structure_free(s);
            return None;
        }
        Some(s)
    }

    /// Structure_Place
    pub fn structure_place(&mut self, s: u16, position: u16) -> bool {
        if position == 0xFFFF {
            return false;
        }
        let si = *self.si(s);
        let hid = self.s(s).o.house_id;
        let player = self.player_house_id;
        match self.s(s).o.type_ {
            STRUCTURE_WALL => {
                if self.structure_is_valid_build_location(position, STRUCTURE_WALL) == 0 {
                    return false;
                }
                let p = position as usize;
                self.map[p].ground_tile_id = self.wall_tile_id + 1;
                self.map[p].house_id = hid;
                self.map_tile_id[p] |= 0x8000;
                if hid == player {
                    self.tile_remove_fog_in_radius(Tile32::unpack(position), 1);
                }
                if self.map_is_position_unveiled(position) {
                    self.map[p].overlay_tile_id = 0;
                }
                self.structure_connect_wall(position, true);
                self.structure_free(s);
                return true;
            }
            STRUCTURE_SLAB_1X1 | STRUCTURE_SLAB_2X2 => {
                let layout = si.layout as usize;
                let mut result = false;
                let passes = if self.s(s).o.type_ == STRUCTURE_SLAB_2X2 { 2 } else { 1 };
                for pass in 0..passes {
                    for i in 0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
                        let cur = position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][i]);
                        if self.structure_is_valid_build_location(cur, STRUCTURE_SLAB_1X1) == 0 {
                            continue;
                        }
                        let c = (cur & 0xFFF) as usize;
                        self.map[c].ground_tile_id = self.built_slab_tile_id;
                        self.map[c].house_id = hid;
                        self.map_tile_id[c] |= 0x8000;
                        if hid == player {
                            self.tile_remove_fog_in_radius(Tile32::unpack(cur), 1);
                            if pass == 1 {
                                self.map[c].overlay_tile_id = 0;
                            }
                        }
                        if pass == 0 && self.map_is_position_unveiled(cur) {
                            self.map[c].overlay_tile_id = 0;
                        }
                        result = true;
                    }
                }
                if !result {
                    return false;
                }
                self.structure_free(s);
                return true;
            }
            _ => {}
        }

        let valid = self.structure_is_valid_build_location(position, self.s(s).o.type_);
        if valid == 0 && hid == player && self.validate_strict_if_zero == 0 {
            return false;
        }
        if !self.config.dune2_enhanced && hid == player {
            self.tile_remove_fog_in_radius(Tile32::unpack(position), 2);
        }
        {
            let st = self.sm(s);
            st.o.seen_by_houses |= 1 << hid;
            if hid == player {
                st.o.seen_by_houses |= 0xFF;
            }
            st.o.flags.is_not_on_map = false;
            let mut p = Tile32::unpack(position);
            p.x &= 0xFF00;
            p.y &= 0xFF00;
            st.o.position = p;
            st.rotation_sprite_diff = 0;
            st.o.hitpoints = si.o.hitpoints;
            st.hitpoints_max = si.o.hitpoints;
        }
        if valid < 0 {
            let tiles_without_slab = (-valid) as u32;
            let count = STRUCTURE_LAYOUT_TILE_COUNT[si.layout as usize] as u32;
            let st = self.sm(s);
            st.o.hitpoints -= ((si.o.hitpoints / 2) as u32 * tiles_without_slab / count) as u16;
            st.o.flags.degrades = true;
        } else if !self.config.dune2_enhanced {
            self.sm(s).o.flags.degrades = true;
        }
        {
            let st = self.sm(s);
            st.o.script.reset(ScriptKind::Structure);
            st.o.script.variables[0] = 0;
            st.o.script.variables[4] = 0;
            st.o.script.delay = 0;
        }
        let t = self.s(s).o.type_;
        self.script_load(Ctx::Structure(s), t);

        let layout = si.layout as usize;
        for i in 0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
            let cur = position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][i]);
            if let Some(u) = self.unit_get_by_packed_tile(cur) {
                self.unit_remove(u);
            }
            if self.config.dune2_enhanced && hid == player {
                self.tile_remove_fog_in_radius(Tile32::unpack(cur), 2);
            }
        }
        if t == STRUCTURE_WINDTRAP {
            self.hm(hid).windtrap_count += 1;
        }
        if self.validate_strict_if_zero == 0 {
            self.house_calculate_power_and_credit(hid);
        }
        self.structure_update_map(s);
        let b = self.structure_get_structures_built(hid);
        self.hm(hid).structures_built = b;
        true
    }

    /// Structure_CalculateHitpointsMax
    pub fn structure_calculate_hitpoints_max(&mut self, h: u8) {
        if h == self.player_house_id {
            self.house_update_radar_state(h);
        }
        let house = self.h(h);
        let power: u32 = if house.power_usage == 0 { 256 } else { (house.power_production as u32 * 256 / house.power_usage as u32).min(256) };
        let mut find = PoolFind::new(h, 0xFFFF);
        while let Some(s) = self.structure_find(&mut find) {
            if is_slab_or_wall(self.s(s).o.type_) {
                continue;
            }
            let hp = self.si(s).o.hitpoints;
            let mut max = (hp as u32 * power / 256) as u16;
            max = max.max(hp / 2);
            self.sm(s).hitpoints_max = max;
            if max >= self.s(s).o.hitpoints {
                continue;
            }
            self.structure_damage(s, 1, 0);
        }
    }

    /// Structure_SetState
    pub fn structure_set_state(&mut self, s: u16, state: i16) {
        // ENHANCEMENT -- Structure_UpdateMap restarts the structure's animation even when the
        //  state does not change. The outpost (and silo) script ends in a return that reloads it,
        //  so it sets its own state again about every 45 ticks and the radar dish never gets past
        //  its second frame. Keep a running animation when nothing changes.
        if self.config.dune2_enhanced && self.s(s).state == state && !self.s(s).o.flags.is_not_on_map
            && self.map[self.s(s).o.position.pack() as usize].has_animation
        {
            return;
        }
        self.sm(s).state = state;
        self.structure_update_map(s);
    }

    /// Structure_Get_ByPackedTile
    pub fn structure_get_by_packed_tile(&self, packed: u16) -> Option<u16> {
        if crate::tile::is_out_of_map(packed) {
            return None;
        }
        let t = &self.map[packed as usize];
        if !t.has_structure {
            return None;
        }
        Some(t.index as u16 - 1)
    }

    /// Structure_GetStructuresBuilt
    pub fn structure_get_structures_built(&mut self, h: u8) -> u32 {
        let mut result = 0u32;
        self.hm(h).windtrap_count = 0;
        let mut find = PoolFind::new(h, 0xFFFF);
        while let Some(s) = self.structure_find(&mut find) {
            let st = self.s(s);
            if st.o.flags.is_not_on_map || is_slab_or_wall(st.o.type_) {
                continue;
            }
            result |= 1 << st.o.type_;
            if st.o.type_ == STRUCTURE_WINDTRAP {
                self.hm(h).windtrap_count += 1;
            }
        }
        result
    }

    /// Structure_IsValidBuildLocation (player rules: must touch an own structure/slab).
    pub fn structure_is_valid_build_location(&self, position: u16, type_: u8) -> i16 {
        self.structure_is_valid_build_location_for(position, type_, self.player_house_id)
    }

    /// Structure_IsValidBuildLocation, generalised to any house (the original always checks
    /// adjacency against g_playerHouseID; passing the player house gives identical results).
    pub fn structure_is_valid_build_location_for(&self, position: u16, type_: u8, house: u8) -> i16 {
        let si = &self.structure_info[type_ as usize];
        let layout = si.layout as usize;
        let mut is_valid = true;
        let mut needed_slabs: i16 = 0;
        for i in 0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
            let cur = position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][i]);
            let lst = self.map_get_landscape_type(cur);
            if !self.map_is_valid_position(cur) {
                is_valid = false;
                break;
            }
            if si.o.flags.not_on_concrete {
                if !LANDSCAPE_INFO[lst as usize].is_valid_for_structure2 && self.validate_strict_if_zero == 0 {
                    is_valid = false;
                    break;
                }
            } else {
                if !LANDSCAPE_INFO[lst as usize].is_valid_for_structure && self.validate_strict_if_zero == 0 {
                    is_valid = false;
                    break;
                }
                if lst != LST_CONCRETE_SLAB {
                    needed_slabs += 1;
                }
            }
            if self.object_get_by_packed_tile(cur).is_some() {
                is_valid = false;
                break;
            }
        }
        if self.validate_strict_if_zero == 0 && is_valid && type_ != STRUCTURE_CONSTRUCTION_YARD {
            is_valid = false;
            for i in 0..16 {
                let offset = STRUCTURE_LAYOUT_TILES_AROUND[layout][i];
                if offset == 0 {
                    break;
                }
                let cur = position.wrapping_add(offset as u16);
                if let Some(s) = self.structure_get_by_packed_tile(cur) {
                    if self.s(s).o.house_id != house {
                        continue;
                    }
                    is_valid = true;
                    break;
                }
                let lst = self.map_get_landscape_type(cur);
                if lst != LST_CONCRETE_SLAB && lst != LST_WALL {
                    continue;
                }
                if self.map[(cur & 0xFFF) as usize].house_id != house {
                    continue;
                }
                is_valid = true;
                break;
            }
        }
        if !is_valid {
            return 0;
        }
        if needed_slabs == 0 {
            return 1;
        }
        -needed_slabs
    }

    /// Structure_ActivateSpecial
    pub fn structure_activate_special(&mut self, s: u16) {
        if self.s(s).o.type_ != STRUCTURE_PALACE {
            return;
        }
        let hid = self.s(s).o.house_id;
        if !self.h(hid).flags.used {
            return;
        }
        match HOUSE_INFO[hid as usize].special_weapon {
            HOUSE_WEAPON_MISSILE => {
                self.validate_strict_if_zero += 1;
                let o = self.rng.random_256() as i8;
                let u = self.unit_create(INDEX_INVALID, UNIT_MISSILE_HOUSE, hid, Tile32::new(0xFFFF, 0xFFFF), o);
                self.validate_strict_if_zero -= 1;
                self.unit_house_missile = u;
                let Some(u) = u else { return };
                self.sm(s).count_down = HOUSE_INFO[hid as usize].special_count_down;
                if !self.h(hid).flags.human {
                    let mut find = PoolFind::all();
                    while let Some(sf) = self.structure_find(&mut find) {
                        if is_slab_or_wall(self.s(sf).o.type_) {
                            continue;
                        }
                        if self.house_are_allied(hid, self.s(sf).o.house_id) {
                            continue;
                        }
                        let p = self.s(sf).o.position.pack();
                        self.unit_launch_house_missile(p);
                        return;
                    }
                    self.unit_free(u);
                    self.unit_house_missile = None;
                    return;
                }
                self.house_missile_countdown = 7;
                self.change_selection_type(SelectionType::Target);
            }
            HOUSE_WEAPON_FREMEN => {
                // The original makes these house Fremen, which it has allied with Atreides. In skirmish the
                // Fremen (the worms' house) are nobody's ally (see house_are_allied), so the troops belong to the
                // palace's own house instead.
                let owner = if self.skirmish.enabled { hid } else { HOUSE_FREMEN };
                let location = self.map_find_location_tile(4, HOUSE_INVALID);
                for _ in 0..5 {
                    self.rng.random_256();
                    let position = self.tile_move_by_random(Tile32::unpack(location), 32, true);
                    let orientation = self.rng.lcg_range(0, 3);
                    let unit_type = if orientation == 1 { UNIT_TROOPER } else { UNIT_TROOPERS };
                    self.validate_strict_if_zero += 1;
                    let u = self.unit_create(INDEX_INVALID, unit_type, owner, position, orientation as i8);
                    self.validate_strict_if_zero -= 1;
                    if let Some(u) = u {
                        self.unit_set_action(u, ACTION_HUNT);
                    }
                }
                self.sm(s).count_down = HOUSE_INFO[hid as usize].special_count_down;
            }
            HOUSE_WEAPON_SABOTEUR => {
                let position = self.structure_find_free_position(s, false);
                if position == 0 {
                    self.sm(s).count_down = 1;
                    return;
                }
                self.validate_strict_if_zero += 1;
                let o = self.rng.random_256() as i8;
                let u = self.unit_create(INDEX_INVALID, UNIT_SABOTEUR, hid, Tile32::unpack(position), o);
                self.validate_strict_if_zero -= 1;
                let Some(u) = u else { return };
                self.unit_set_action(u, ACTION_SABOTAGE);
                self.sm(s).count_down = HOUSE_INFO[hid as usize].special_count_down;
            }
            _ => {}
        }
    }

    /// Structure_RemoveFog
    pub fn structure_remove_fog(&mut self, s: u16) {
        if self.s(s).o.house_id != self.player_house_id {
            return;
        }
        let si = *self.si(s);
        let mut position = self.s(s).o.position;
        if self.config.dune2_enhanced {
            let (w, h) = crate::tables::STRUCTURE_LAYOUT_SIZE[si.layout as usize];
            position.x = position.x.wrapping_add(256 * (w - 1) / 2);
            position.y = position.y.wrapping_add(256 * (h - 1) / 2);
        }
        self.tile_remove_fog_in_radius(position, si.o.fog_uncover_radius);
    }

    /// Structure_Destroy
    fn structure_destroy(&mut self, s: u16) {
        {
            let st = self.sm(s);
            st.o.script.variables[0] = 1;
            st.o.flags.allocated = false;
            st.o.flags.repairing = false;
            st.o.script.delay = 0;
            st.o.script.reset(ScriptKind::Structure);
        }
        let t = self.s(s).o.type_;
        self.script_load(Ctx::Structure(s), t);
        let pos = self.s(s).o.position;
        self.voice_at_tile(44, pos);
        let mut linked = self.s(s).o.linked_id;
        if linked != 0xFF {
            if t == STRUCTURE_CONSTRUCTION_YARD {
                self.structure_destroy(linked as u16);
                self.sm(s).o.linked_id = 0xFF;
            } else {
                while linked != 0xFF {
                    let u = linked as u16;
                    linked = self.u(u).o.linked_id;
                    self.unit_remove(u);
                }
            }
        }
        let hid = self.s(s).o.house_id;
        let si = *self.si(s);
        let h = self.h(hid);
        let loss = if h.credits_storage == 0 {
            h.credits
        } else {
            h.credits.min(((h.credits as u32 * 256 / h.credits_storage as u32) * si.credits_storage as u32 / 256) as u16)
        };
        self.hm(hid).credits -= loss;
        if hid != self.player_house_id {
            let bonus = si.o.build_credits + if self.campaign_id > 7 { si.o.build_credits / 2 } else { 0 };
            self.hm(hid).credits = self.h(hid).credits.wrapping_add(bonus);
        }
        if t != STRUCTURE_WINDTRAP {
            return;
        }
        self.hm(hid).windtrap_count = self.h(hid).windtrap_count.wrapping_sub(1);
    }

    /// Structure_Damage
    pub fn structure_damage(&mut self, s: u16, damage: u16, range: u16) -> bool {
        if damage == 0 {
            return false;
        }
        if self.s(s).o.script.variables[0] == 1 {
            return false;
        }
        let si = *self.si(s);
        let hp = self.s(s).o.hitpoints;
        self.sm(s).o.hitpoints = hp.saturating_sub(damage);
        if self.s(s).o.hitpoints == 0 {
            let score = (si.o.build_credits / 100).max(1);
            let hid = self.s(s).o.house_id;
            if self.house_are_allied(self.player_house_id, hid) {
                self.scenario.destroyed_allied += 1;
                self.scenario.score = self.scenario.score.wrapping_sub(score);
            } else {
                self.scenario.destroyed_enemy += 1;
                self.scenario.score = self.scenario.score.wrapping_add(score);
            }
            self.structure_destroy(s);
            if self.player_house_id == hid {
                let index = match hid {
                    HOUSE_HARKONNEN => 22,
                    HOUSE_ATREIDES => 23,
                    HOUSE_ORDOS => 24,
                    _ => 0xFFFF,
                };
                self.feedback(index);
            } else {
                self.feedback(21);
            }
            self.structure_untarget_me(s);
            return true;
        }
        if range == 0 {
            return false;
        }
        let p = add_tile_diff(self.s(s).o.position, STRUCTURE_LAYOUT_TILE_DIFF[si.layout as usize]);
        self.map_make_explosion(EXPLOSION_IMPACT_LARGE, p, 0, 0);
        false
    }

    /// Structure_IsUpgradable
    pub fn structure_is_upgradable(&self, s: u16) -> bool {
        let st = self.s(s);
        let si = self.si(s);
        let hid = st.o.house_id;
        if hid == HOUSE_HARKONNEN && st.o.type_ == STRUCTURE_HIGH_TECH {
            return false;
        }
        if hid == HOUSE_ORDOS && st.o.type_ == STRUCTURE_HEAVY_VEHICLE && st.upgrade_level == 1 && si.upgrade_campaign[2] > self.campaign_id {
            return false;
        }
        let lvl = st.upgrade_level as usize;
        if lvl < 3 && si.upgrade_campaign[lvl] != 0 && si.upgrade_campaign[lvl] <= self.campaign_id + 1 {
            if st.o.type_ != STRUCTURE_CONSTRUCTION_YARD {
                return true;
            }
            if st.upgrade_level != 1 {
                return true;
            }
            let req = self.structure_info[STRUCTURE_ROCKET_TURRET as usize].o.structures_required;
            return self.h(hid).structures_built & req == req;
        }
        hid == HOUSE_HARKONNEN && st.o.type_ == STRUCTURE_WOR_TROOPER && st.upgrade_level == 0 && self.campaign_id > 3
    }

    /// Structure_ConnectWall
    pub fn structure_connect_wall(&mut self, position: u16, recurse: bool) -> bool {
        const WALL: [u8; 256] = [
            0, 3, 1, 2, 3, 3, 4, 5, 1, 6, 1, 7, 8, 9, 10, 11, 1, 12, 1, 19, 1, 16, 1, 31, 1, 28, 1, 52, 1, 45, 1, 59, 3, 3, 13, 20, 3, 3,
            22, 32, 3, 3, 13, 53, 3, 3, 38, 60, 5, 6, 7, 21, 5, 6, 7, 33, 5, 6, 7, 54, 5, 6, 7, 61, 9, 9, 9, 9, 17, 17, 23, 34, 9, 9, 9,
            9, 25, 46, 39, 62, 11, 12, 11, 12, 13, 18, 13, 35, 11, 12, 11, 12, 13, 47, 13, 63, 15, 15, 16, 16, 17, 17, 24, 36, 15, 15,
            16, 16, 17, 17, 40, 64, 19, 20, 21, 22, 23, 24, 25, 37, 19, 20, 21, 22, 23, 24, 25, 65, 27, 27, 27, 27, 27, 27, 27, 27, 14,
            29, 14, 55, 26, 48, 41, 66, 29, 30, 29, 30, 29, 30, 29, 30, 31, 30, 31, 56, 31, 49, 31, 67, 33, 33, 34, 34, 33, 33, 34, 34,
            35, 35, 15, 57, 35, 35, 42, 68, 37, 38, 39, 40, 37, 38, 39, 40, 41, 42, 43, 58, 41, 42, 43, 69, 45, 45, 45, 45, 46, 46, 46,
            46, 47, 47, 47, 47, 27, 50, 43, 70, 49, 50, 49, 50, 51, 52, 51, 52, 53, 54, 53, 54, 55, 51, 55, 71, 57, 57, 58, 58, 59, 59,
            60, 60, 61, 61, 62, 62, 63, 63, 44, 72, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 73,
        ];
        let mut bits: u16 = 0;
        let is_destroyed_wall = self.map_get_landscape_type(position) == LST_DESTROYED_WALL;
        for i in 0..4 {
            let cur = position.wrapping_add(MAP_DIFF[i] as u16);
            if recurse && self.map_get_landscape_type(cur) == LST_WALL {
                self.structure_connect_wall(cur, false);
            }
            if is_destroyed_wall {
                continue;
            }
            match self.map_get_landscape_type(cur) {
                LST_DESTROYED_WALL => bits |= (1 << (i + 4)) | (1 << i),
                LST_WALL => bits |= 1 << i,
                _ => {}
            }
        }
        if is_destroyed_wall {
            return false;
        }
        let tile_id = self.wall_tile_id + WALL[bits as usize] as u16 + 1;
        let p = (position & 0xFFF) as usize;
        if self.map[p].ground_tile_id == tile_id {
            return false;
        }
        self.map[p].ground_tile_id = tile_id;
        self.map_tile_id[p] |= 0x8000;
        true
    }

    /// Structure_UntargetMe
    pub fn structure_untarget_me(&mut self, s: u16) {
        let encoded = self.structure_encode(s);
        self.object_script_variable4_clear(ObjRef::Structure(s));
        let mut find = PoolFind::all();
        while let Some(u) = self.unit_find(&mut find) {
            if self.u(u).target_move == encoded {
                self.um(u).target_move = 0;
            }
            if self.u(u).target_attack == encoded {
                self.um(u).target_attack = 0;
            }
            if self.u(u).o.script.variables[4] == encoded {
                self.object_script_variable4_clear(ObjRef::Unit(u));
            }
        }
        let mut find = PoolFind::all();
        while let Some(t) = self.team_find(&mut find) {
            if self.teams[t as usize].target == encoded {
                self.teams[t as usize].target = 0;
            }
        }
    }

    /// Structure_FindFreePosition
    pub fn structure_find_free_position(&mut self, s: u16, check_for_spice: bool) -> u16 {
        let layout = self.si(s).layout as usize;
        let packed = self.s(s).o.position.center().pack();
        let spice_packed = if check_for_spice { self.map_search_spice(packed, 10) } else { 0 };
        let mut best_packed = 0u16;
        let mut best_distance = 0u16;
        let mut i = (self.rng.random_256() & 0xF) as usize;
        for _ in 0..16 {
            let offset = STRUCTURE_LAYOUT_TILES_AROUND[layout][i];
            i = (i + 1) & 0xF;
            if offset == 0 {
                continue;
            }
            let cur = packed.wrapping_add(offset as u16);
            if !self.map_is_valid_position(cur) {
                continue;
            }
            let t = self.map_get_landscape_type(cur);
            if t == LST_WALL || t == LST_ENTIRELY_MOUNTAIN || t == LST_PARTIAL_MOUNTAIN {
                continue;
            }
            let tile = &self.map[cur as usize];
            if tile.has_unit || tile.has_structure {
                continue;
            }
            if !check_for_spice {
                return cur;
            }
            if best_distance == 0 || get_distance_packed(cur, spice_packed) < best_distance {
                best_packed = cur;
                best_distance = get_distance_packed(cur, spice_packed);
            }
        }
        best_packed
    }

    /// Structure_Remove
    pub fn structure_remove(&mut self, s: u16) {
        let si = *self.si(s);
        let packed = self.s(s).o.position.pack();
        let layout = si.layout as usize;
        for i in 0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
            let cur = packed.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][i]);
            self.animation_stop_by_tile(cur);
            self.map[(cur & 0xFFF) as usize].has_structure = false;
        }
        let pos = self.s(s).o.position;
        let hid = self.s(s).o.house_id;
        self.animation_start(Some((AnimTable::Structure, 0)), pos, si.layout, hid, si.icon_group as u8);
        for i in 0..5 {
            if self.h(hid).ai_structure_rebuild[i][0] != 0 {
                continue;
            }
            let t = self.s(s).o.type_ as u16;
            self.hm(hid).ai_structure_rebuild[i] = [t, packed];
            break;
        }
        self.structure_free(s);
        self.structure_untarget_me(s);
        let b = self.structure_get_structures_built(hid);
        self.hm(hid).structures_built = b;
        self.house_update_credits_storage(hid);
        match self.s(s).o.type_ {
            STRUCTURE_WINDTRAP => self.house_calculate_power_and_credit(hid),
            STRUCTURE_OUTPOST => {
                self.house_update_radar_state(hid);
            }
            _ => {}
        }
    }

    /// Structure_CheckAvailableConcrete
    pub fn structure_check_available_concrete(&self, structure_type: u16, house_id: u8) -> bool {
        let si = &self.structure_info[structure_type as usize];
        let layout = si.layout as usize;
        if structure_type == STRUCTURE_SLAB_1X1 as u16 || structure_type == STRUCTURE_SLAB_2X2 as u16 {
            return true;
        }
        for i in 0..4096u16 {
            let stop = (0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize).all(|j| {
                let p = i.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]);
                self.map_get_landscape_type(p) == LST_CONCRETE_SLAB && self.map[(p & 0xFFF) as usize].house_id == house_id
            });
            if stop {
                return true;
            }
        }
        false
    }

    /// Structure_CancelBuild
    pub fn structure_cancel_build(&mut self, s: u16) {
        if self.s(s).o.linked_id == 0xFF {
            return;
        }
        let linked = self.s(s).o.linked_id as u16;
        let oi = if self.s(s).o.type_ == STRUCTURE_CONSTRUCTION_YARD {
            let oi = self.si(linked).o;
            self.structure_free(linked);
            oi
        } else {
            let oi = self.ui(linked).o;
            self.unit_free(linked);
            oi
        };
        let hid = self.s(s).o.house_id;
        let refund = (((oi.build_time as u32).wrapping_sub((self.s(s).count_down >> 8) as u32) * 256 / oi.build_time as u32) * oi.build_credits as u32 / 256) as u16;
        self.hm(hid).credits = self.h(hid).credits.wrapping_add(refund);
        let st = self.sm(s);
        st.o.flags.on_hold = false;
        st.count_down = 0;
        st.o.linked_id = 0xFF;
    }

    /// Structure_BuildObject. objectType 0xFFFF opens the factory window (returns false and
    /// emits Event::FactoryWindow; the UI then calls `structure_factory_result`).
    pub fn structure_build_object(&mut self, s: u16, object_type: u16) -> bool {
        let si = *self.si(s);
        if !si.o.flags.factory {
            return false;
        }
        self.structure_set_repairing_state(s, 0);
        if object_type == 0xFFFD {
            self.structure_set_upgrading_state(s, 1);
            return false;
        }
        let mut object_type = object_type;
        if object_type == 0xFFFF || object_type == 0xFFFE {
            let mut upgrade_cost: u16 = 0;
            if self.structure_is_upgradable(s) && si.o.hitpoints == self.s(s).o.hitpoints {
                upgrade_cost = (si.o.build_credits + (si.o.build_credits >> 15)) / 2;
            }
            let t = self.s(s).o.type_;
            let hid = self.s(s).o.house_id;
            if upgrade_cost != 0 && t == STRUCTURE_HIGH_TECH && hid == HOUSE_HARKONNEN {
                upgrade_cost = 0;
            }
            if t == STRUCTURE_STARPORT {
                upgrade_cost = 0;
            }
            let buildable = self.structure_get_buildable(s);
            if buildable == 0 {
                self.sm(s).object_type = 0;
                return false;
            }
            if t == STRUCTURE_CONSTRUCTION_YARD {
                self.factory_window_construction_yard = true;
                for i in 0..STRUCTURE_MAX {
                    if buildable & (1 << i) == 0 {
                        continue;
                    }
                    self.structure_info[i].o.available = 1;
                    if object_type != 0xFFFE {
                        continue;
                    }
                    self.sm(s).object_type = i as u16;
                    return false;
                }
            } else {
                self.factory_window_construction_yard = false;
                if t == STRUCTURE_STARPORT {
                    self.structure_starport_update_available(s);
                } else {
                    for i in 0..UNIT_MAX {
                        if buildable & (1 << i) == 0 {
                            continue;
                        }
                        self.unit_info[i].o.available = 1;
                        if object_type != 0xFFFE {
                            continue;
                        }
                        self.sm(s).object_type = i as u16;
                        return false;
                    }
                }
            }
            if object_type == 0xFFFF {
                let cy = self.factory_window_construction_yard;
                self.emit(Event::FactoryWindow { structure: s, construction_yard: cy, starport: t == STRUCTURE_STARPORT, upgrade_cost });
                return false;
            } else {
                self.sm(s).object_type = object_type;
            }
        }
        if self.s(s).o.type_ == STRUCTURE_STARPORT {
            return true;
        }
        if self.s(s).object_type != object_type {
            self.structure_cancel_build(s);
        }
        if self.s(s).o.linked_id != 0xFF || object_type == 0xFFFF {
            return false;
        }
        let hid = self.s(s).o.house_id;
        let (o, oi) = if self.s(s).o.type_ != STRUCTURE_CONSTRUCTION_YARD {
            let oi = self.unit_info[object_type as usize].o;
            (self.unit_create(INDEX_INVALID, object_type as u8, hid, Tile32::new(0xFFFF, 0xFFFF), 0), oi)
        } else {
            let oi = self.structure_info[object_type as usize].o;
            (self.structure_create(INDEX_INVALID, object_type as u8, hid, 0xFFFF), oi)
        };
        self.sm(s).o.flags.on_hold = false;
        if let Some(o) = o {
            {
                let st = self.sm(s);
                st.o.linked_id = (o & 0xFF) as u8;
                st.object_type = object_type;
                st.count_down = oi.build_time << 8;
            }
            self.structure_set_state(s, STRUCTURE_STATE_BUSY);
            if hid != self.player_house_id {
                return true;
            }
            let fmt = self.string(STR_PRODUCTION_OF_S_HAS_STARTED);
            let name = self.string(oi.string_id_full);
            let text = format_c(&fmt, &[Arg::S(&name)]);
            self.display_text(text, 2);
            return true;
        }
        if hid != self.player_house_id {
            return false;
        }
        object_type = 0;
        let _ = object_type;
        self.display_string(STR_UNABLE_TO_CREATE_MORE, 2);
        false
    }

    /// The starport part of Structure_BuildObject: compute how many of each unit can be ordered.
    fn structure_starport_update_available(&mut self, s: u16) {
        let hid = self.s(s).o.house_id;
        let mut linked_id: u8 = 0xFF;
        let mut available_units = [0i16; UNIT_MAX];
        loop {
            let mut again = false;
            for i in 0..UNIT_MAX {
                let at_starport = self.starport_available[i];
                if at_starport == 0 {
                    self.unit_info[i].o.available = 0;
                } else if at_starport < 0 {
                    self.unit_info[i].o.available = -1;
                } else if at_starport > available_units[i] {
                    self.validate_strict_if_zero += 1;
                    let u = self.unit_allocate(INDEX_INVALID, i as u8, hid);
                    self.validate_strict_if_zero -= 1;
                    if let Some(u) = u {
                        again = true;
                        self.um(u).o.linked_id = linked_id;
                        linked_id = (u & 0xFF) as u8;
                        available_units[i] += 1;
                        self.unit_info[i].o.available = available_units[i] as i8;
                    } else if available_units[i] == 0 {
                        self.unit_info[i].o.available = -1;
                    }
                }
            }
            if !again {
                break;
            }
        }
        while linked_id != 0xFF {
            let u = linked_id as u16;
            linked_id = self.u(u).o.linked_id;
            self.unit_free(u);
        }
    }

    /// Handle the result of a factory window (the rest of Structure_BuildObject(s, 0xFFFF)).
    pub fn structure_factory_result(&mut self, s: u16, res: FactoryResult) -> bool {
        match res {
            FactoryResult::Resume => false,
            FactoryResult::Upgrade => {
                self.structure_set_upgrading_state(s, 1);
                false
            }
            FactoryResult::Buy(items) => {
                let hid = self.s(s).o.house_id;
                let mut object_type = 0xFFFFu16;
                let mut chosen = None;
                for (ot, amount) in items {
                    if amount == 0 {
                        continue;
                    }
                    object_type = ot;
                    if self.s(s).o.type_ != STRUCTURE_STARPORT {
                        self.structure_cancel_build(s);
                        self.sm(s).object_type = object_type;
                        chosen = Some(ot);
                        continue;
                    }
                    for _ in 0..amount {
                        self.validate_strict_if_zero += 1;
                        let u = self.unit_create(INDEX_INVALID, object_type as u8, hid, Tile32::new(0xFFFF, 0xFFFF), 0);
                        self.validate_strict_if_zero -= 1;
                        let Some(u) = u else {
                            let c = self.unit_info[UNIT_CARRYALL as usize].o.build_credits;
                            self.hm(hid).credits = self.h(hid).credits.wrapping_add(c);
                            if hid == self.player_house_id {
                                self.display_string(STR_UNABLE_TO_CREATE_MORE, 2);
                            }
                            continue;
                        };
                        self.structure_index = s;
                        if self.h(hid).starport_time_left == 0 {
                            self.hm(hid).starport_time_left = HOUSE_INFO[hid as usize].starport_delivery_time;
                        }
                        let link = self.h(hid).starport_linked_id;
                        self.um(u).o.linked_id = (link & 0xFF) as u8;
                        self.hm(hid).starport_linked_id = u;
                        let ot = object_type as usize;
                        self.starport_available[ot] -= 1;
                        if self.starport_available[ot] <= 0 {
                            self.starport_available[ot] = -1;
                        }
                    }
                }
                if self.s(s).o.type_ == STRUCTURE_STARPORT {
                    return true;
                }
                let Some(ot) = chosen else { return false };
                // For construction yards, the original warns about missing concrete here.
                if self.factory_window_construction_yard && !self.structure_check_available_concrete(ot, hid) {
                    self.emit(Event::Hint(STR_THERE_ISNT_ENOUGH_OPEN_CONCRETE_TO_PLACE_THIS_STRUCTURE_YOU_MAY_PROCEED_BUT_WITHOUT_ENOUGH_CONCRETE_THE_BUILDING_WILL_NEED_REPAIRS, self.structure_info[ot as usize].o.sprite_id));
                }
                let _ = object_type;
                self.structure_build_object(s, ot)
            }
        }
    }

    /// Structure_SetUpgradingState
    pub fn structure_set_upgrading_state(&mut self, s: u16, state: i8) -> bool {
        let mut ret = false;
        let mut state = state;
        if state == -1 {
            state = if self.s(s).o.flags.upgrading { 0 } else { 1 };
        }
        let player = self.s(s).o.house_id == self.player_house_id;
        if state == 0 && self.s(s).o.flags.upgrading {
            if player {
                self.display_string(STR_UPGRADING_STOPS, 2);
            }
            self.sm(s).o.flags.upgrading = false;
            self.sm(s).o.flags.on_hold = false;
            ret = true;
        }
        if state == 0 || self.s(s).o.flags.upgrading || self.s(s).upgrade_time_left == 0 {
            return ret;
        }
        if player {
            self.display_string(STR_UPGRADING_STARTS, 2);
        }
        let keep_repairing = self.config.repair_while_upgrading;
        let st = self.sm(s);
        st.o.flags.on_hold = true;
        st.o.flags.repairing &= keep_repairing;
        st.o.flags.upgrading = true;
        true
    }

    /// Structure_SetRepairingState
    pub fn structure_set_repairing_state(&mut self, s: u16, state: i8) -> bool {
        let mut ret = false;
        if self.config.dune2_enhanced && self.s(s).o.flags.upgrading && !self.config.repair_while_upgrading {
            return false;
        }
        let mut state = state;
        if !self.s(s).o.flags.allocated {
            state = 0;
        }
        if state == -1 {
            state = if self.s(s).o.flags.repairing { 0 } else { 1 };
        }
        let player = self.s(s).o.house_id == self.player_house_id;
        if state == 0 && self.s(s).o.flags.repairing {
            if player {
                self.display_string(STR_REPAIRING_STOPS, 2);
            }
            self.sm(s).o.flags.repairing = false;
            self.sm(s).o.flags.on_hold = false;
            ret = true;
        }
        if state == 0 || self.s(s).o.flags.repairing || self.s(s).o.hitpoints == self.si(s).o.hitpoints {
            return ret;
        }
        if player {
            self.display_string(STR_REPAIRING_STARTS, 2);
        }
        self.sm(s).o.flags.on_hold = true;
        self.sm(s).o.flags.repairing = true;
        true
    }

    /// Structure_UpdateMap
    pub fn structure_update_map(&mut self, s: u16) {
        let st = self.s(s);
        if !st.o.flags.used || st.o.flags.is_not_on_map {
            return;
        }
        let si = *self.si(s);
        let layout = si.layout as usize;
        let size = STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize;
        let base = self.icon_map[si.icon_group as usize] as usize + size + size;
        let pos = st.o.position;
        let hid = st.o.house_id;
        let rot = st.rotation_sprite_diff;
        let state = st.state;
        for i in 0..size {
            let p = (pos.pack().wrapping_add(STRUCTURE_LAYOUT_TILES[layout][i]) & 0xFFF) as usize;
            let tile_id = self.icon_map[base + i].wrapping_add(rot);
            let unveiled = self.tile_is_unveiled(self.map[p].overlay_tile_id);
            let t = &mut self.map[p];
            t.house_id = hid;
            t.has_structure = true;
            t.index = (s + 1) as u8;
            t.ground_tile_id = tile_id;
            if unveiled {
                t.overlay_tile_id = 0;
            }
        }
        self.sm(s).o.flags.is_dirty = true;
        if state >= STRUCTURE_STATE_IDLE {
            let idx = state.min(STRUCTURE_STATE_READY) as usize;
            if si.animation_index[idx] == 0xFF {
                self.animation_start(None, pos, si.layout, hid, si.icon_group as u8);
            } else {
                let id = si.animation_index[idx] as u16;
                self.animation_start(Some((AnimTable::Structure, id)), pos, si.layout, hid, si.icon_group as u8);
            }
        } else {
            self.animation_start(Some((AnimTable::Structure, 1)), pos, si.layout, hid, si.icon_group as u8);
        }
    }

    /// Structure_GetBuildable
    pub fn structure_get_buildable(&mut self, s: u16) -> u32 {
        let si = *self.si(s);
        let hid = self.s(s).o.house_id;
        let creator = self.s(s).creator_house_id as u8;
        let structures_built = self.h(hid).structures_built;
        let upgrade_level = self.s(s).upgrade_level;
        let upgrade_time_left = self.s(s).upgrade_time_left;
        let mut ret: u32 = 0;
        match self.s(s).o.type_ {
            STRUCTURE_LIGHT_VEHICLE | STRUCTURE_HEAVY_VEHICLE | STRUCTURE_HIGH_TECH | STRUCTURE_WOR_TROOPER | STRUCTURE_BARRACKS => {
                for ui in self.unit_info.iter_mut() {
                    ui.o.available = 0;
                }
                for i in 0..8 {
                    let mut unit_type = si.buildable_units[i];
                    if unit_type == UNIT_INVALID {
                        continue;
                    }
                    if unit_type == UNIT_TRIKE && creator == HOUSE_ORDOS {
                        unit_type = UNIT_RAIDER_TRIKE;
                    }
                    let ui = &mut self.unit_info[unit_type as usize];
                    let mut level_required = ui.o.upgrade_level_required as u16;
                    if unit_type == UNIT_SIEGE_TANK && creator == HOUSE_ORDOS {
                        level_required = level_required.wrapping_sub(1);
                    }
                    if structures_built & ui.o.structures_required != ui.o.structures_required {
                        continue;
                    }
                    if ui.o.available_house & (1 << creator) == 0 {
                        continue;
                    }
                    if upgrade_level as u16 >= level_required {
                        ui.o.available = 1;
                        ret |= 1 << unit_type;
                        continue;
                    }
                    if upgrade_time_left != 0 && upgrade_level as u16 + 1 >= level_required {
                        ui.o.available = -1;
                    }
                }
                ret
            }
            STRUCTURE_CONSTRUCTION_YARD => {
                let player = self.player_house_id;
                let campaign = self.campaign_id;
                for i in 0..STRUCTURE_MAX {
                    let local = &mut self.structure_info[i];
                    local.o.available = 0;
                    let mut available_campaign = local.o.available_campaign;
                    let mut structures_required = local.o.structures_required;
                    if i == STRUCTURE_WOR_TROOPER as usize && hid == HOUSE_HARKONNEN && campaign >= 1 {
                        structures_required &= !(1 << STRUCTURE_BARRACKS);
                        available_campaign = 2;
                    }
                    if structures_built & structures_required == structures_required || hid != player {
                        if hid != HOUSE_HARKONNEN && i == STRUCTURE_LIGHT_VEHICLE as usize {
                            available_campaign = 2;
                        }
                        if campaign as i32 >= available_campaign as i32 - 1 && local.o.available_house & (1 << hid) != 0 {
                            if upgrade_level >= local.o.upgrade_level_required || hid != player {
                                local.o.available = 1;
                                ret |= 1 << i;
                            } else if upgrade_time_left != 0 && upgrade_level + 1 >= local.o.upgrade_level_required {
                                local.o.available = -1;
                            }
                        }
                    }
                }
                ret
            }
            STRUCTURE_STARPORT => 0xFFFF_FFFF,
            _ => 0,
        }
    }

    /// Structure_HouseUnderAttack
    pub fn structure_house_under_attack(&mut self, house_id: u8) {
        if house_id != self.player_house_id && self.h(house_id).flags.done_full_scale_attack {
            return;
        }
        self.hm(house_id).flags.done_full_scale_attack = true;
        if self.h(house_id).flags.human {
            if self.h(house_id).timer_structure_attack != 0 {
                return;
            }
            self.feedback(48);
            self.hm(house_id).timer_structure_attack = 8;
            return;
        }
        if !self.config.dune2_enhanced {
            return;
        }
        let mut find = PoolFind::new(house_id, 0xFFFF);
        while let Some(u) = self.unit_find(&mut find) {
            if self.ui(u).bullet_type == UNIT_INVALID {
                continue;
            }
            if self.u(u).action_id == ACTION_AREA_GUARD {
                self.unit_set_action(u, ACTION_HUNT);
            }
        }
    }

    /// Structure_AI_PickNextToBuild
    pub fn structure_ai_pick_next_to_build(&mut self, s: u16) -> u16 {
        let hid = self.s(s).o.house_id;
        // NOTE: the original stores the result in a uint16, dropping unit types >= 16.
        let full = self.structure_get_buildable(s);
        let mut buildable = full as u16 as u32;
        let t = self.s(s).o.type_;
        if t == STRUCTURE_CONSTRUCTION_YARD {
            // Skirmish: the truncation above would drop the rocket turret, silo and outpost (structures 16 to 18),
            // which the computer's build order wants. Structure types do not need it.
            let buildable = full;
            self.skirmish_drop_blocked_rebuilds(hid, true);
            for i in 0..5 {
                let ty = self.h(hid).ai_structure_rebuild[i][0];
                if ty == 0 {
                    continue;
                }
                if buildable & (1 << ty) == 0 {
                    continue;
                }
                return ty;
            }
            let picked = self.skirmish_ai_pick_structure(hid, buildable);
            self.skirmish_plan_placement(hid, picked);
            return picked;
        }
        if t == STRUCTURE_HIGH_TECH {
            let mut find = PoolFind::new(hid, UNIT_CARRYALL as u16);
            while self.unit_find(&mut find).is_some() {
                buildable &= !(1 << UNIT_CARRYALL);
            }
        }
        if t == STRUCTURE_HEAVY_VEHICLE {
            // Skirmish extension: computer houses also replace/expand their harvesters.
            if self.skirmish_ai_want_harvester(hid) && self.structure_get_buildable(s) & (1 << UNIT_HARVESTER) != 0 {
                return UNIT_HARVESTER as u16;
            }
            buildable &= !(1 << UNIT_HARVESTER);
            buildable &= !(1 << UNIT_MCV);
            buildable = self.skirmish_ai_affordable(hid, buildable);
        }
        let mut type_: u16 = 0xFFFF;
        for i in 0..UNIT_MAX as u16 {
            if buildable & (1 << i) == 0 {
                continue;
            }
            if self.rng.random_256() % 4 == 0 {
                type_ = i;
            }
            if type_ != 0xFFFF && self.unit_info[i as usize].o.priority_build <= self.unit_info[type_ as usize].o.priority_build {
                continue;
            }
            type_ = i;
        }
        type_
    }

    pub fn icon_group_landscape(&self) -> usize {
        self.icon_map[ICM_ICONGROUP_LANDSCAPE] as usize
    }
}

use crate::tables::StructureInfo;
