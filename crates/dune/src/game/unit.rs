//! Units (unit.c).

use super::map::*;
use super::script::Ctx;
use super::*;
use crate::tables::landscape::LANDSCAPE_INFO;
use crate::tables::{ACTION_INFO, ANIMATION_UNIT_MOVE, HOUSE_INFO};

pub const SCRIPT_UNIT_OPCODES_PER_TICK: i32 = 50;

impl Game {
    /// Unit_Rotate
    fn unit_rotate(&mut self, u: u16, level: usize) {
        let unit = self.um(u);
        if unit.orientation[level].speed == 0 {
            return;
        }
        let target = unit.orientation[level].target;
        let current = unit.orientation[level].current;
        let mut diff = target as i16 - current as i16;
        if diff > 128 {
            diff -= 256;
        }
        if diff < -128 {
            diff += 256;
        }
        let diff = diff.abs();
        let mut new_current = current.wrapping_add(unit.orientation[level].speed);
        if (unit.orientation[level].speed as i16).abs() >= diff {
            unit.orientation[level].speed = 0;
            new_current = target;
        }
        unit.orientation[level].current = new_current;
    }

    /// Unit_MovementTick
    fn unit_movement_tick(&mut self, u: u16) {
        if self.u(u).speed == 0 {
            return;
        }
        let mut speed = self.u(u).speed_remainder as u16;
        if self.ui(u).movement_type != MOVEMENT_WINGER {
            speed += self.adjust_to_game_speed(self.u(u).speed_per_tick as u16, 1, 255, false);
        } else {
            speed += self.u(u).speed_per_tick as u16;
        }
        if speed & 0xFF00 != 0 {
            let d = (self.u(u).speed as u16 * 16).min(get_distance(self.u(u).o.position, self.u(u).current_destination).wrapping_add(16));
            self.unit_move(u, d);
        }
        self.um(u).speed_remainder = (speed & 0xFF) as u8;
    }

    /// GameLoop_Unit
    pub fn game_loop_unit(&mut self) {
        let mut tick_movement = false;
        let mut tick_rotation = false;
        let mut tick_blinking = false;
        let mut tick_unknown4 = false;
        let mut tick_script = false;
        let mut tick_unknown5 = false;
        let mut tick_deviation = false;
        let now = self.timer_game;

        if self.ticks.unit_movement <= now {
            tick_movement = true;
            self.ticks.unit_movement = now + 3;
        }
        if self.ticks.unit_rotation <= now {
            tick_rotation = true;
            self.ticks.unit_rotation = now + self.adjust_to_game_speed(4, 2, 8, true) as u32;
        }
        if self.ticks.unit_blinking <= now {
            tick_blinking = true;
            self.ticks.unit_blinking = now + 3;
        }
        if self.ticks.unit_unknown4 <= now {
            tick_unknown4 = true;
            self.ticks.unit_unknown4 = now + 20;
        }
        if self.ticks.unit_script <= now {
            tick_script = true;
            self.ticks.unit_script = now + 5;
        }
        if self.ticks.unit_unknown5 <= now {
            tick_unknown5 = true;
            self.ticks.unit_unknown5 = now + 5;
        }
        if self.ticks.unit_deviation <= now {
            tick_deviation = true;
            self.ticks.unit_deviation = now + 60;
        }

        let mut find = PoolFind::all();
        while let Some(u) = self.unit_find(&mut find) {
            let ui = *self.ui(u);
            self.script_current_object = Some(ObjRef::Unit(u));
            self.script_current_structure = None;
            self.script_current_unit = Some(u);
            self.script_current_team = None;

            if self.u(u).o.flags.is_not_on_map {
                continue;
            }

            if tick_unknown4 && self.u(u).target_attack != 0 && ui.o.flags.has_turret {
                let tile = self.index_get_tile(self.u(u).target_attack);
                let d = get_direction(self.u(u).o.position, tile);
                self.unit_set_orientation(u, d, false, 1);
            }

            if tick_movement {
                self.unit_movement_tick(u);
                if self.u(u).fire_delay != 0 {
                    if ui.movement_type == MOVEMENT_WINGER && !ui.flags.is_normal_unit {
                        let mut tile = self.u(u).current_destination;
                        let ta = self.u(u).target_attack;
                        if Self::index_get_type(ta) == IT_UNIT {
                            if let Some(tu) = self.index_get_unit(ta) {
                                if self.ui(tu).movement_type == MOVEMENT_WINGER {
                                    tile = self.index_get_tile(ta);
                                }
                            }
                        }
                        let d = get_direction(self.u(u).o.position, tile);
                        self.unit_set_orientation(u, d, false, 0);
                    }
                    self.um(u).fire_delay -= 1;
                }
            }

            if tick_rotation {
                self.unit_rotate(u, 0);
                if ui.o.flags.has_turret {
                    self.unit_rotate(u, 1);
                }
            }

            if tick_blinking && self.u(u).blink_counter != 0 {
                let unit = self.um(u);
                unit.blink_counter -= 1;
                unit.o.flags.is_highlighted = unit.blink_counter % 2 != 0;
            }

            if tick_deviation {
                self.unit_deviation_decrease(u, 1);
            }

            if ui.movement_type != MOVEMENT_WINGER && self.object_get_by_packed_tile(self.u(u).o.position.pack()).is_none() {
                self.unit_update_map(1, u);
            }

            if tick_unknown5 {
                if self.u(u).timer == 0 {
                    if (ui.movement_type == MOVEMENT_FOOT && self.u(u).speed != 0) || self.u(u).o.flags.is_smoking {
                        if self.u(u).sprite_offset >= 0 {
                            let unit = self.um(u);
                            unit.sprite_offset &= 0x3F;
                            unit.sprite_offset += 1;
                            unit.timer = ui.animation_speed / 5;
                            if unit.o.flags.is_smoking {
                                unit.timer = 3;
                                if unit.sprite_offset > 32 {
                                    unit.o.flags.is_smoking = false;
                                    unit.sprite_offset = 0;
                                }
                            }
                        }
                    }
                    if self.u(u).o.type_ == UNIT_ORNITHOPTER && self.u(u).o.flags.allocated && self.u(u).sprite_offset >= 0 {
                        let unit = self.um(u);
                        unit.sprite_offset &= 0x3F;
                        unit.sprite_offset += 1;
                        unit.timer = 1;
                    }
                    if self.u(u).o.type_ == UNIT_HARVESTER {
                        let unit = self.um(u);
                        if unit.action_id == ACTION_HARVEST || unit.o.flags.is_smoking {
                            unit.sprite_offset &= 0x3F;
                            unit.sprite_offset += 1;
                            unit.timer = 4;
                        } else if unit.sprite_offset != 0 {
                            unit.sprite_offset = 0;
                        }
                    }
                } else {
                    self.um(u).timer -= 1;
                }
            }

            if tick_script {
                if self.u(u).o.script.delay == 0 {
                    if self.u(u).o.script.is_loaded() {
                        let mut opcodes_left = SCRIPT_UNIT_OPCODES_PER_TICK + 2;
                        if !ui.o.flags.script_no_slowdown && !self.map_is_position_in_viewport(self.u(u).o.position).0 {
                            opcodes_left = 3;
                        }
                        self.um(u).o.script.variables[3] = self.player_house_id as u16;
                        while opcodes_left > 0 && self.u(u).o.script.delay == 0 {
                            if !self.script_run(Ctx::Unit(u)) {
                                break;
                            }
                            opcodes_left -= 1;
                        }
                    }
                } else {
                    self.um(u).o.script.delay -= 1;
                }
            }

            if self.u(u).next_action_id == ACTION_INVALID {
                continue;
            }
            if self.u(u).current_destination.x != 0 || self.u(u).current_destination.y != 0 {
                continue;
            }
            let na = self.u(u).next_action_id;
            self.unit_set_action(u, na);
            self.um(u).next_action_id = ACTION_INVALID;
        }
    }

    /// Unit_GetHouseID
    pub fn unit_get_house_id(&self, u: u16) -> u8 {
        let unit = &self.units[u as usize];
        if unit.deviated != 0 {
            if self.config.dune2_enhanced {
                return unit.deviated_house;
            }
            return HOUSE_ORDOS;
        }
        unit.o.house_id
    }

    /// Unit_Create
    pub fn unit_create(&mut self, index: u16, type_id: u8, house_id: u8, position: Tile32, orientation: i8) -> Option<u16> {
        if house_id >= HOUSE_MAX || type_id as usize >= UNIT_MAX {
            return None;
        }
        let ui = self.unit_info[type_id as usize];
        let u = self.unit_allocate(index, type_id, house_id)?;
        self.um(u).o.house_id = house_id;
        self.unit_set_orientation(u, orientation, true, 0);
        self.unit_set_orientation(u, orientation, true, 1);
        self.unit_set_speed(u, 0);
        {
            let unit = self.um(u);
            unit.o.position = position;
            unit.o.hitpoints = ui.o.hitpoints;
            unit.current_destination = Tile32::default();
            unit.origin_encoded = 0;
            unit.route[0] = 0xFF;
        }
        if position.x != 0xFFFF || position.y != 0xFFFF {
            self.unit_find_closest_refinery(u);
            let unit = self.um(u);
            unit.target_last = position;
            unit.target_pre_last = position;
        }
        {
            let unit = self.um(u);
            unit.o.linked_id = 0xFF;
            unit.o.script.delay = 0;
            unit.action_id = ACTION_GUARD;
            unit.next_action_id = ACTION_INVALID;
            unit.fire_delay = 0;
            unit.distance_to_destination = 0x7FFF;
            unit.target_move = 0;
            unit.amount = 0;
            unit.wobble_index = 0;
            unit.sprite_offset = 0;
            unit.blink_counter = 0;
            unit.timer = 0;
            unit.o.script.reset(ScriptKind::Unit);
            unit.o.flags.allocated = true;
        }
        if ui.movement_type == MOVEMENT_TRACKED && (self.rng.random_256() as u16) < HOUSE_INFO[house_id as usize].degrading_chance {
            self.um(u).o.flags.degrades = true;
        }
        if ui.movement_type == MOVEMENT_WINGER {
            self.unit_set_speed(u, 255);
        } else if (position.x != 0xFFFF || position.y != 0xFFFF) && self.unit_is_tile_occupied(u) {
            self.unit_free(u);
            return None;
        }
        if position.x == 0xFFFF && position.y == 0xFFFF {
            self.um(u).o.flags.is_not_on_map = true;
            return Some(u);
        }
        self.unit_update_map(1, u);
        let action = if house_id == self.player_house_id { ui.o.actions_player[3] as u8 } else { ui.action_ai as u8 };
        self.unit_set_action(u, action);
        Some(u)
    }

    /// Unit_IsTypeOnMap
    pub fn unit_is_type_on_map(&self, house_id: u8, type_id: u8) -> bool {
        self.unit_find.iter().any(|&u| {
            let unit = self.u(u);
            !(house_id != HOUSE_INVALID && self.unit_get_house_id(u) != house_id)
                && !(type_id != UNIT_INVALID && unit.o.type_ != type_id)
                && !(self.validate_strict_if_zero == 0 && unit.o.flags.is_not_on_map)
        })
    }

    /// Unit_SetAction
    pub fn unit_set_action(&mut self, u: u16, action: u8) {
        let a = self.u(u).action_id;
        if a == ACTION_DESTRUCT || a == ACTION_DIE || action == ACTION_INVALID {
            return;
        }
        let ai = &ACTION_INFO[action as usize];
        match ai.switch_type {
            0 | 1 => {
                if ai.switch_type == 0 && (self.u(u).current_destination.x != 0 || self.u(u).current_destination.y != 0) {
                    self.um(u).next_action_id = action;
                    return;
                }
                let unit = self.um(u);
                unit.action_id = action;
                unit.next_action_id = ACTION_INVALID;
                unit.current_destination = Tile32::default();
                unit.o.script.delay = 0;
                unit.o.script.reset(ScriptKind::Unit);
                unit.o.script.variables[0] = action as u16;
                let t = unit.o.type_;
                self.script_load(Ctx::Unit(u), t);
            }
            2 => {
                self.um(u).o.script.variables[0] = action as u16;
                let t = self.u(u).o.type_;
                self.script_load_as_subroutine(Ctx::Unit(u), t);
            }
            _ => {}
        }
    }

    /// Unit_AddToTeam
    pub fn unit_add_to_team(&mut self, u: u16, t: u16) -> u16 {
        self.um(u).team = (t + 1) as u8;
        let team = &mut self.teams[t as usize];
        team.members += 1;
        team.max_members.wrapping_sub(team.members)
    }

    /// Unit_RemoveFromTeam
    pub fn unit_remove_from_team(&mut self, u: u16) -> u16 {
        if self.u(u).team == 0 {
            return 0;
        }
        let t = self.u(u).team as usize - 1;
        self.teams[t].members = self.teams[t].members.wrapping_sub(1);
        self.um(u).team = 0;
        self.teams[t].max_members.wrapping_sub(self.teams[t].members)
    }

    /// Unit_GetTeam
    pub fn unit_get_team(&self, u: u16) -> Option<u16> {
        let t = self.u(u).team;
        (t != 0).then(|| t as u16 - 1)
    }

    /// Unit_Sort: bubble pass to keep draw order by Y, and recount visible units.
    pub fn unit_sort(&mut self) {
        let p = self.player_house_id;
        if p == HOUSE_INVALID {
            return;
        }
        self.hm(p).unit_count_enemy = 0;
        self.hm(p).unit_count_allied = 0;
        let n = self.unit_find.len();
        for i in 0..n.saturating_sub(1) {
            let u1 = self.unit_find[i];
            let u2 = self.unit_find[i + 1];
            let mut y1 = self.u(u1).o.position.y;
            let mut y2 = self.u(u2).o.position.y;
            if self.ui(u1).movement_type == MOVEMENT_FOOT {
                y1 = y1.wrapping_sub(0x100);
            }
            if self.ui(u2).movement_type == MOVEMENT_FOOT {
                y2 = y2.wrapping_sub(0x100);
            }
            if (y1 as i16) > (y2 as i16) {
                self.unit_find[i] = u2;
                self.unit_find[i + 1] = u1;
            }
        }
        for i in 0..n {
            let u = self.unit_find[i];
            let unit = self.u(u);
            if unit.o.seen_by_houses & (1 << p) != 0 && !unit.o.flags.is_not_on_map {
                if self.house_are_allied(unit.o.house_id, p) {
                    self.hm(p).unit_count_allied += 1;
                } else {
                    self.hm(p).unit_count_enemy += 1;
                }
            }
        }
    }

    /// Unit_Get_ByPackedTile
    pub fn unit_get_by_packed_tile(&self, packed: u16) -> Option<u16> {
        if crate::tile::is_out_of_map(packed) {
            return None;
        }
        let t = &self.map[packed as usize];
        if !t.has_unit {
            return None;
        }
        Some(t.index as u16 - 1)
    }

    /// Unit_IsValidMovementIntoStructure
    pub fn unit_is_valid_movement_into_structure(&self, u: u16, s: u16) -> u16 {
        let si = self.si(s);
        let ui = self.ui(u);
        let unit_enc = self.unit_encode(u);
        let struct_enc = self.structure_encode(s);
        let unit = self.u(u);
        if self.unit_get_house_id(u) != self.s(s).o.house_id {
            if unit.o.type_ == UNIT_SABOTEUR && unit.target_move == struct_enc {
                return 2;
            }
            if ui.movement_type == MOVEMENT_FOOT && si.o.flags.conquerable {
                return if unit.target_move == struct_enc { 2 } else { 1 };
            }
            return 0;
        }
        if si.enter_filter & (1 << unit.o.type_) == 0 {
            return 0;
        }
        if self.s(s).o.script.variables[4] == unit_enc {
            return 2;
        }
        if self.s(s).o.linked_id == 0xFF {
            1
        } else {
            0
        }
    }

    /// Unit_SetDestination
    pub fn unit_set_destination(&mut self, u: u16, destination: u16) {
        if !self.index_is_valid(destination) {
            return;
        }
        if self.u(u).target_move == destination {
            return;
        }
        let mut destination = destination;
        if Self::index_get_type(destination) == IT_TILE {
            let packed = Self::index_decode(destination);
            if let Some(u2) = self.unit_get_by_packed_tile(packed) {
                if u != u2 {
                    destination = self.unit_encode(u2);
                }
            } else if let Some(s) = self.structure_get_by_packed_tile(packed) {
                destination = self.structure_encode(s);
            }
        }
        if let Some(s) = self.index_get_structure(destination) {
            if self.s(s).o.house_id == self.unit_get_house_id(u)
                && (self.unit_is_valid_movement_into_structure(u, s) == 1 || self.ui(u).movement_type == MOVEMENT_WINGER)
            {
                let ue = self.unit_encode(u);
                self.object_script_variable4_link(ue, destination);
            }
        }
        let unit = self.um(u);
        unit.target_move = destination;
        unit.route[0] = 0xFF;
    }

    /// Unit_GetTargetUnitPriority
    pub fn unit_get_target_unit_priority(&self, u: u16, target: u16) -> u16 {
        if u == target {
            return 0;
        }
        let t = self.u(target);
        if !t.o.flags.allocated {
            return 0;
        }
        if !self.ai_sees(t.o.seen_by_houses, self.unit_get_house_id(u)) {
            return 0;
        }
        if self.house_are_allied(self.unit_get_house_id(u), self.unit_get_house_id(target)) {
            return 0;
        }
        let unit_info = self.ui(u);
        let target_info = self.ui(target);
        if !target_info.o.flags.priority {
            return 0;
        }
        if target_info.movement_type == MOVEMENT_WINGER {
            if !unit_info.o.flags.target_air {
                return 0;
            }
            if t.o.house_id == self.player_house_id && !self.map_is_position_unveiled(t.o.position.pack()) {
                return 0;
            }
        }
        if !self.map_is_valid_position(t.o.position.pack()) {
            return 0;
        }
        let distance = get_distance_rounded_up(self.u(u).o.position, t.o.position);
        if !self.map_is_valid_position(self.u(u).o.position.pack()) && target_info.fire_distance >= distance {
            return 0;
        }
        let mut priority = target_info.o.priority_target.wrapping_add(target_info.o.priority_build);
        if distance != 0 {
            priority = (priority / distance) + 1;
        }
        priority.min(0x7D00)
    }

    /// Unit_FindClosestRefinery
    pub fn unit_find_closest_refinery(&mut self, u: u16) -> u16 {
        let res = if self.u(u).origin_encoded == 0 { 0 } else { 1 };
        if self.u(u).o.type_ != UNIT_HARVESTER {
            let e = self.tile_encode(self.u(u).o.position.pack());
            self.um(u).origin_encoded = e;
            return res;
        }
        let h = self.unit_get_house_id(u);
        let pos = self.u(u).o.position;
        let mut s: Option<u16> = None;
        let mut mind: u16 = 0;
        for s2 in self.structures_of(h, STRUCTURE_REFINERY as u16) {
            if self.s(s2).state != STRUCTURE_STATE_BUSY {
                continue;
            }
            let d = get_distance(pos, self.s(s2).o.position);
            if mind != 0 && d >= mind {
                continue;
            }
            mind = d;
            s = Some(s2);
        }
        if s.is_none() {
            for s2 in self.structures_of(h, STRUCTURE_REFINERY as u16) {
                let d = get_distance(pos, self.s(s2).o.position);
                if mind != 0 && d >= mind {
                    continue;
                }
                mind = d;
                s = Some(s2);
            }
        }
        if let Some(s) = s {
            let e = self.structure_encode(s);
            self.um(u).origin_encoded = e;
        }
        res
    }

    /// Unit_SetPosition
    pub fn unit_set_position(&mut self, u: u16, position: Tile32) -> bool {
        let ui = *self.ui(u);
        {
            let unit = self.um(u);
            unit.o.flags.is_not_on_map = false;
            unit.o.position = position.center();
        }
        if self.u(u).origin_encoded == 0 {
            self.unit_find_closest_refinery(u);
        }
        self.um(u).o.script.variables[4] = 0;
        if self.unit_is_tile_occupied(u) {
            self.um(u).o.flags.is_not_on_map = true;
            return false;
        }
        {
            let unit = self.um(u);
            unit.current_destination = Tile32::default();
            unit.target_move = 0;
            unit.target_attack = 0;
        }
        if self.map[self.u(u).o.position.pack() as usize].is_unveiled {
            let h = self.u(u).o.house_id;
            self.um(u).o.seen_by_houses &= !(1 << h);
            let p = self.player_house_id;
            self.unit_house_unit_count_add(u, p);
        }
        let t = self.u(u).o.type_;
        if self.u(u).o.house_id != self.player_house_id || t == UNIT_HARVESTER || t == UNIT_SABOTEUR {
            self.unit_set_action(u, ui.action_ai as u8);
        } else {
            self.unit_set_action(u, ui.o.actions_player[3] as u8);
        }
        self.um(u).sprite_offset = 0;
        self.unit_update_map(1, u);
        true
    }

    /// Unit_Remove
    pub fn unit_remove(&mut self, u: u16) {
        self.um(u).o.flags.allocated = true;
        self.unit_untarget_me(u);
        if self.unit_selected == Some(u) {
            self.unit_select(None);
        }
        self.um(u).o.flags.bullet_is_big = true;
        self.unit_update_map(0, u);
        self.unit_house_unit_count_remove(u);
        self.um(u).o.script.reset(ScriptKind::Unit);
        self.unit_free(u);
    }

    /// Unit_FindBestTargetUnit
    pub fn unit_find_best_target_unit(&mut self, u: u16, mode: u16) -> Option<u16> {
        let mut position = self.u(u).o.position;
        if self.u(u).origin_encoded == 0 {
            let e = self.tile_encode(position.pack());
            self.um(u).origin_encoded = e;
        } else {
            position = self.index_get_tile(self.u(u).origin_encoded);
        }
        let mut distance = self.ui(u).fire_distance << 8;
        if mode == 2 {
            distance <<= 1;
        }
        let mut best = None;
        let mut best_priority: u16 = 0;
        let mut find = PoolFind::all();
        while let Some(target) = self.unit_find(&mut find) {
            if mode != 0 && mode != 4 {
                if mode == 1 && get_distance(self.u(u).o.position, self.u(target).o.position) > distance {
                    continue;
                }
                if mode == 2 && get_distance(position, self.u(target).o.position) > distance {
                    continue;
                }
            }
            let priority = self.unit_get_target_unit_priority(u, target);
            if (priority as i16) > (best_priority as i16) {
                best = Some(target);
                best_priority = priority;
            }
        }
        if best_priority == 0 {
            return None;
        }
        best
    }

    fn unit_sandworm_get_target_priority(&self, u: u16, target: u16) -> u16 {
        let tpos = self.u(target).o.position;
        if !self.map_is_position_unveiled(tpos.pack()) {
            return 0;
        }
        if !self.landscape_info(tpos.pack()).is_sand {
            return 0;
        }
        let mut res: u16 = match self.ui(target).movement_type {
            MOVEMENT_FOOT => 0x64,
            MOVEMENT_TRACKED => 0x3E8,
            MOVEMENT_HARVESTER => 0x3E8,
            MOVEMENT_WHEELED => 0x1388,
            _ => 0,
        };
        if self.u(target).speed != 0 || self.u(target).fire_delay != 0 {
            res = res.wrapping_mul(4);
        }
        let distance = get_distance_rounded_up(self.u(u).o.position, tpos);
        if distance != 0 && res != 0 {
            res /= distance;
        }
        if distance < 2 {
            res = res.wrapping_mul(2);
        }
        res
    }

    /// Unit_Sandworm_FindBestTarget
    pub fn unit_sandworm_find_best_target(&self, u: u16) -> Option<u16> {
        let mut best = None;
        let mut best_priority = 0u16;
        let mut find = PoolFind::all();
        while let Some(t) = self.unit_find(&mut find) {
            let p = self.unit_sandworm_get_target_priority(u, t);
            if p >= best_priority {
                best = Some(t);
                best_priority = p;
            }
        }
        if best_priority == 0 {
            return None;
        }
        best
    }

    /// Unit_StartMovement
    pub fn unit_start_movement(&mut self, u: u16) -> bool {
        let ui = *self.ui(u);
        let orientation = ((self.u(u).orientation[0].current as i16 + 16) & 0xE0) as u8 as i8;
        self.unit_set_orientation(u, orientation, true, 0);
        self.unit_set_orientation(u, orientation, false, 1);
        let position = move_by_orientation(self.u(u).o.position, orientation as u8);
        let packed = position.pack();
        self.um(u).distance_to_destination = 0x7FFF;
        let score = self.unit_get_tile_enter_score(u, packed, (orientation as u8 / 32) as u16);
        if score > 255 || score == -1 {
            return false;
        }
        let mut type_ = self.map_get_landscape_type(packed);
        if type_ == LST_STRUCTURE {
            type_ = LST_CONCRETE_SLAB;
        }
        let li = &LANDSCAPE_INFO[type_ as usize];
        let mut speed = li.movement_speed[ui.movement_type as usize] as u16;
        if self.u(u).o.type_ == UNIT_SABOTEUR && type_ == LST_WALL {
            speed = 255;
        }
        self.um(u).o.flags.is_smoking = false;
        if self.config.dune2_enhanced {
            self.um(u).o.flags.is_wobbling = li.let_unit_wobble;
        } else if li.let_unit_wobble {
            self.um(u).o.flags.is_wobbling = true;
        }
        if ui.o.hitpoints / 2 > self.u(u).o.hitpoints && ui.movement_type != MOVEMENT_WINGER {
            speed -= speed / 4;
        }
        self.unit_set_speed(u, speed);
        if ui.movement_type != MOVEMENT_SLITHER {
            let old = self.u(u).o.position;
            self.um(u).o.position = position;
            self.unit_update_map(1, u);
            self.um(u).o.position = old;
        }
        self.um(u).current_destination = position;
        self.unit_deviation_decrease(u, 10);
        true
    }

    /// Unit_SetTarget
    pub fn unit_set_target(&mut self, u: u16, encoded: u16) {
        if !self.index_is_valid(encoded) {
            return;
        }
        if self.u(u).target_attack == encoded {
            return;
        }
        let mut encoded = encoded;
        if Self::index_get_type(encoded) == IT_TILE {
            let packed = Self::index_decode(encoded);
            if let Some(t) = self.unit_get_by_packed_tile(packed) {
                encoded = self.unit_encode(t);
            } else if let Some(s) = self.structure_get_by_packed_tile(packed) {
                encoded = self.structure_encode(s);
            }
        }
        if self.unit_encode(u) == encoded {
            encoded = self.tile_encode(self.u(u).o.position.pack());
        }
        self.um(u).target_attack = encoded;
        if !self.ui(u).o.flags.has_turret {
            let unit = self.um(u);
            unit.target_move = encoded;
            unit.route[0] = 0xFF;
        }
    }

    /// Unit_Deviation_Decrease
    pub fn unit_deviation_decrease(&mut self, u: u16, amount: u16) -> bool {
        if self.u(u).deviated == 0 {
            return false;
        }
        let ui = *self.ui(u);
        if !ui.flags.is_normal_unit {
            return false;
        }
        let amount = if amount == 0 { HOUSE_INFO[self.u(u).o.house_id as usize].toughness } else { amount };
        if self.u(u).deviated as u16 > amount {
            self.um(u).deviated -= amount as u8;
            return false;
        }
        self.um(u).deviated = 0;
        if self.u(u).o.house_id == self.player_house_id {
            self.unit_set_action(u, ui.o.actions_player[3] as u8);
        } else {
            self.unit_set_action(u, ui.action_ai as u8);
        }
        self.unit_untarget_me(u);
        self.um(u).target_attack = 0;
        self.um(u).target_move = 0;
        true
    }

    /// Unit_RemoveFog
    pub fn unit_remove_fog(&mut self, u: u16) {
        let unit = self.u(u);
        if unit.o.flags.is_not_on_map {
            return;
        }
        let p = unit.o.position;
        if (p.x == 0xFFFF && p.y == 0xFFFF) || (p.x == 0 && p.y == 0) {
            return;
        }
        if !self.house_are_allied(self.unit_get_house_id(u), self.player_house_id) {
            return;
        }
        let r = self.ui(u).o.fog_uncover_radius;
        if r == 0 {
            return;
        }
        self.tile_remove_fog_in_radius(p, r);
    }

    /// Unit_Deviate
    pub fn unit_deviate(&mut self, u: u16, probability: u16, house_id: u8) -> bool {
        let ui = *self.ui(u);
        if !ui.flags.is_normal_unit || self.u(u).deviated != 0 || ui.flags.is_not_deviatable {
            return false;
        }
        let mut probability = if probability == 0 { HOUSE_INFO[self.u(u).o.house_id as usize].toughness } else { probability };
        if self.u(u).o.house_id != self.player_house_id {
            probability -= probability / 8;
        }
        if self.rng.random_256() as u16 >= probability {
            return false;
        }
        self.um(u).deviated = 120;
        self.um(u).deviated_house = house_id;
        if self.player_house_id == house_id {
            self.unit_set_action(u, ui.o.actions_player[3] as u8);
        } else {
            self.unit_set_action(u, ui.action_ai as u8);
        }
        self.unit_untarget_me(u);
        self.um(u).target_attack = 0;
        self.um(u).target_move = 0;
        true
    }

    /// Unit_Move
    pub fn unit_move(&mut self, u: u16, distance: u16) -> bool {
        if !self.u(u).o.flags.used {
            return false;
        }
        let ui = *self.ui(u);
        let mut new_position = move_by_direction(self.u(u).o.position, self.u(u).orientation[0].current as i16, distance);
        if new_position == self.u(u).o.position {
            return false;
        }
        if !new_position.is_valid() {
            if !ui.flags.must_stay_in_map {
                self.unit_remove(u);
                return true;
            }
            if self.u(u).o.flags.by_scenario && self.u(u).o.linked_id == 0xFF && self.u(u).o.script.variables[4] == 0 {
                self.unit_remove(u);
                return true;
            }
            new_position = self.u(u).o.position;
            let o = self.u(u).orientation[0].current.wrapping_add((self.rng.random_256() & 0xF) as i8);
            self.unit_set_orientation(u, o, false, 0);
        }
        self.um(u).wobble_index = 0;
        if ui.flags.can_wobble && self.u(u).o.flags.is_wobbling {
            self.um(u).wobble_index = self.rng.random_256() & 7;
        }
        let d = get_distance(new_position, self.u(u).current_destination);
        let packed = new_position.pack();

        if ui.flags.is_tracked && d < 48 {
            let other = self.unit_get_by_packed_tile(packed);
            if let Some(o) = other.filter(|&o| self.ui(o).movement_type == MOVEMENT_FOOT && self.u(o).o.flags.allocated) {
                if self.unit_selected == Some(o) {
                    self.unit_select(None);
                }
                self.unit_untarget_me(o);
                self.um(o).o.script.variables[1] = 1;
                self.unit_set_action(o, ACTION_DIE);
            } else {
                let type_ = self.map_get_landscape_type(packed);
                if (type_ == LST_NORMAL_SAND || type_ == LST_ENTIRELY_DUNE) && self.map[packed as usize].overlay_tile_id == 0 {
                    let animation_id = orientation_256_to_8(self.u(u).orientation[0].current as u8);
                    let pos = self.u(u).o.position;
                    let h = self.u(u).o.house_id;
                    let _ = ANIMATION_UNIT_MOVE;
                    self.animation_start(Some((AnimTable::UnitMove, animation_id as u16)), pos, 0, h, 5);
                }
            }
        }

        self.unit_update_map(0, u);
        if ui.movement_type == MOVEMENT_WINGER {
            let f = &mut self.um(u).o.flags;
            f.animation_flip = !f.animation_flip;
        }

        let current_destination = self.u(u).current_destination;
        let distance = get_distance(new_position, current_destination);
        let ret;
        let mut is_spice_bloom = false;

        if self.u(u).o.type_ == UNIT_SONIC_BLAST {
            let damage = (self.u(u).o.hitpoints / 4) + 1;
            ret = false;
            if let Some(t) = self.unit_get_by_packed_tile(packed) {
                if !self.ui(t).flags.sonic_protection {
                    self.unit_damage(t, damage, 0);
                }
            } else if let Some(s) = self.structure_get_by_packed_tile(packed) {
                if self.config.dune2_enhanced
                    && self.s(s).o.house_id != self.player_house_id
                    && !self.house_are_allied(self.u(u).o.house_id, self.s(s).o.house_id)
                {
                    let h = self.s(s).o.house_id;
                    self.structure_house_under_attack(h);
                }
                self.structure_damage(s, damage, 0);
            } else if self.map_get_landscape_type(packed) == LST_WALL && self.structure_info[STRUCTURE_WALL as usize].o.hitpoints > damage {
                self.rng.random_256();
            }
            if self.u(u).o.hitpoints < ui.damage / 2 {
                self.um(u).o.flags.bullet_is_big = true;
            }
            self.um(u).o.hitpoints = self.u(u).o.hitpoints.wrapping_sub(1);
            if self.u(u).o.hitpoints == 0 || self.u(u).fire_delay == 0 {
                self.unit_remove(u);
            }
        } else {
            if self.u(u).o.type_ == UNIT_BULLET {
                let mut type_ = self.map_get_landscape_type(new_position.pack());
                if (type_ == LST_WALL || type_ == LST_STRUCTURE)
                    && Self::index_get_type(self.u(u).origin_encoded) == IT_STRUCTURE
                    && self.map[new_position.pack() as usize].house_id == self.u(u).o.house_id
                {
                    type_ = LST_NORMAL_SAND;
                }
                if type_ == LST_WALL || type_ == LST_STRUCTURE || type_ == LST_ENTIRELY_MOUNTAIN {
                    self.um(u).o.position = new_position;
                    let hp = self.u(u).o.hitpoints;
                    let origin = self.u(u).origin_encoded;
                    self.map_make_explosion((ui.explosion_type + hp / 10) & 3, new_position, hp, origin);
                    self.unit_remove(u);
                    return true;
                }
            }

            ret = self.u(u).distance_to_destination < distance || distance < 16;
            if ret {
                if ui.flags.is_bullet {
                    if self.u(u).fire_delay == 0 || self.u(u).o.type_ == UNIT_MISSILE_TURRET {
                        if self.u(u).o.type_ == UNIT_MISSILE_HOUSE {
                            const OX: [i16; 17] = [0, 0, 200, 256, 200, 0, -200, -256, -200, 0, 400, 512, 400, 0, -400, -512, -400];
                            const OY: [i16; 17] = [0, -256, -200, 0, 200, 256, 200, 0, -200, -512, -400, 0, 400, 512, 400, 0, -400];
                            for i in 0..17 {
                                let p = Tile32 { x: new_position.x.wrapping_add(OX[i] as u16), y: new_position.y.wrapping_add(OY[i] as u16) };
                                if p.is_valid() {
                                    self.map_make_explosion(ui.explosion_type, p, 200, 0);
                                }
                            }
                        } else if ui.explosion_type != 0xFFFF {
                            let upos = self.u(u).o.position;
                            let hp = self.u(u).o.hitpoints;
                            let origin = self.u(u).origin_encoded;
                            if ui.flags.impact_on_sand
                                && self.map[upos.pack() as usize].index == 0
                                && self.map_get_landscape_type(upos.pack()) == LST_NORMAL_SAND
                            {
                                self.map_make_explosion(EXPLOSION_SAND_BURST, new_position, hp, origin);
                            } else if self.u(u).o.type_ == UNIT_MISSILE_DEVIATOR {
                                let h = self.u(u).o.house_id;
                                self.map_deviate_area(ui.explosion_type, new_position, 32, h);
                            } else {
                                self.map_make_explosion((ui.explosion_type + hp / 20) & 3, new_position, hp, origin);
                            }
                        }
                        self.unit_remove(u);
                        return true;
                    }
                } else if ui.flags.is_ground_unit {
                    if current_destination.x != 0 || current_destination.y != 0 {
                        new_position = current_destination;
                    }
                    {
                        let unit = self.um(u);
                        unit.target_pre_last = unit.target_last;
                        unit.target_last = unit.o.position;
                        unit.current_destination = Tile32::default();
                    }
                    if self.u(u).o.flags.degrades && (self.rng.random_256() & 3) == 0 {
                        self.unit_damage(u, 1, 0);
                    }
                    if self.u(u).o.type_ == UNIT_SABOTEUR {
                        let mut detonate = self.map_get_landscape_type(new_position.pack()) == LST_WALL;
                        if !detonate {
                            let tm = self.u(u).target_move;
                            if self.config.dune2_enhanced {
                                detonate = tm != 0 && get_distance(new_position, self.index_get_tile(tm)) < 16;
                            } else {
                                detonate = tm != 0 && get_distance(self.u(u).o.position, self.index_get_tile(tm)) < 32;
                            }
                        }
                        if detonate {
                            self.map_make_explosion(EXPLOSION_SABOTEUR_DEATH, new_position, 500, 0);
                            self.unit_remove(u);
                            return true;
                        }
                    }
                    self.unit_set_speed(u, 0);
                    if self.u(u).target_move == self.tile_encode(packed) {
                        self.um(u).target_move = 0;
                    }
                    if let Some(s) = self.structure_get_by_packed_tile(packed) {
                        let unit = self.um(u);
                        unit.target_pre_last = Tile32::default();
                        unit.target_last = Tile32::default();
                        self.unit_enter_structure(u, s);
                        return true;
                    }
                    if self.u(u).o.type_ != UNIT_SANDWORM {
                        let g = self.map[packed as usize].ground_tile_id;
                        if g == self.bloom_tile_id || g == self.bloom_tile_id + 1 {
                            is_spice_bloom = true;
                        }
                    }
                }
            }
        }

        self.um(u).distance_to_destination = distance;
        self.um(u).o.position = new_position;
        self.unit_update_map(1, u);
        if is_spice_bloom {
            let h = self.unit_get_house_id(u);
            self.map_bloom_explode_spice(packed, h);
        }
        ret
    }

    /// Unit_Damage
    pub fn unit_damage(&mut self, u: u16, damage: u16, range: u16) -> bool {
        if !self.u(u).o.flags.allocated {
            return false;
        }
        let mut ui = *self.ui(u);
        if !ui.flags.is_normal_unit && self.u(u).o.type_ != UNIT_SANDWORM {
            return false;
        }
        let alive = self.u(u).o.hitpoints != 0;
        let hp = self.u(u).o.hitpoints;
        self.um(u).o.hitpoints = hp.saturating_sub(damage);
        self.unit_deviation_decrease(u, 0);
        let house_id = self.unit_get_house_id(u);

        if self.u(u).o.hitpoints == 0 {
            self.unit_remove_player(u);
            if self.u(u).o.type_ == UNIT_HARVESTER {
                let p = self.u(u).o.position.pack();
                let a = self.u(u).amount as u16 / 32;
                self.map_fill_circle_with_spice(p, a);
            }
            if self.u(u).o.type_ == UNIT_SABOTEUR {
                self.feedback(20);
            } else if !ui.o.flags.no_message_on_death && alive {
                let id = if house_id == self.player_house_id || self.campaign_id > 3 { house_id as u16 + 14 } else { 13 };
                self.feedback(id);
            }
            self.unit_set_action(u, ACTION_DIE);
            return true;
        }

        if range != 0 {
            let pos = self.u(u).o.position;
            self.map_make_explosion(if damage < 25 { EXPLOSION_IMPACT_SMALL } else { EXPLOSION_IMPACT_MEDIUM }, pos, 0, 0);
        }

        if house_id != self.player_house_id && self.u(u).action_id == ACTION_AMBUSH && self.u(u).o.type_ != UNIT_HARVESTER {
            self.unit_set_action(u, ACTION_ATTACK);
        }

        if self.u(u).o.hitpoints >= ui.o.hitpoints / 2 {
            return false;
        }

        if self.u(u).o.type_ == UNIT_SANDWORM {
            self.unit_set_action(u, ACTION_DIE);
        }

        if self.u(u).o.type_ == UNIT_TROOPERS || self.u(u).o.type_ == UNIT_INFANTRY {
            self.um(u).o.type_ += 2;
            ui = *self.ui(u);
            self.um(u).o.hitpoints = ui.o.hitpoints;
            if (self.rng.random_256() as u16) < HOUSE_INFO[self.u(u).o.house_id as usize].toughness {
                self.unit_set_action(u, ACTION_RETREAT);
            }
        }

        if ui.movement_type != MOVEMENT_TRACKED && ui.movement_type != MOVEMENT_HARVESTER && ui.movement_type != MOVEMENT_WHEELED {
            return false;
        }
        let unit = self.um(u);
        unit.o.flags.is_smoking = true;
        unit.sprite_offset = 0;
        unit.timer = 0;
        false
    }

    /// Unit_UntargetMe
    pub fn unit_untarget_me(&mut self, u: u16) {
        let encoded = self.unit_encode(u);
        self.object_script_variable4_clear(ObjRef::Unit(u));
        let mut find = PoolFind::all();
        while let Some(o) = self.unit_find(&mut find) {
            if self.u(o).target_move == encoded {
                self.um(o).target_move = 0;
            }
            if self.u(o).target_attack == encoded {
                self.um(o).target_attack = 0;
            }
            if self.u(o).o.script.variables[4] == encoded {
                self.object_script_variable4_clear(ObjRef::Unit(o));
            }
        }
        let mut find = PoolFind::all();
        while let Some(s) = self.structure_find(&mut find) {
            let t = self.s(s).o.type_;
            if t != STRUCTURE_TURRET && t != STRUCTURE_ROCKET_TURRET {
                continue;
            }
            if self.s(s).o.script.variables[2] == encoded {
                self.sm(s).o.script.variables[2] = 0;
            }
        }
        self.unit_remove_from_team(u);
        let mut find = PoolFind::all();
        while let Some(t) = self.team_find(&mut find) {
            if self.teams[t as usize].target == encoded {
                self.teams[t as usize].target = 0;
            }
        }
    }

    /// Unit_SetOrientation
    pub fn unit_set_orientation(&mut self, u: u16, orientation: i8, rotate_instantly: bool, level: usize) {
        let turning = self.ui(u).turning_speed;
        let o = &mut self.um(u).orientation[level];
        o.speed = 0;
        o.target = orientation;
        if rotate_instantly {
            o.current = orientation;
            return;
        }
        if o.current == orientation {
            return;
        }
        o.speed = (turning as i16 * 4) as i8;
        let diff = orientation as i16 - o.current as i16;
        if (diff > -128 && diff < 0) || diff > 128 {
            o.speed = o.speed.wrapping_neg();
        }
    }

    /// Unit_Select
    pub fn unit_select(&mut self, unit: Option<u16>) {
        if unit == self.unit_selected {
            return;
        }
        let mut unit = unit;
        if let Some(u) = unit {
            if !self.u(u).o.flags.allocated {
                unit = None;
            } else if self.u(u).o.seen_by_houses & (1 << self.player_house_id) == 0 {
                unit = None;
            }
        }
        let Some(u) = unit else {
            self.unit_selected = None;
            self.change_selection_type(SelectionType::Structure);
            return;
        };
        if self.unit_get_house_id(u) == self.player_house_id {
            let ui = *self.ui(u);
            self.emit(Event::Sound(if ui.movement_type == MOVEMENT_FOOT { 18 } else { 19 }));
            self.emit(Event::Hint(ui.o.hint_string_id, ui.o.sprite_id));
        }
        if self.unit_selected.is_some() {
            if self.unit_selected != Some(u) {
                self.unit_display_status_text(u);
            }
            self.unit_selected = Some(u);
        } else {
            self.unit_display_status_text(u);
            self.unit_selected = Some(u);
            self.change_selection_type(SelectionType::Unit);
        }
    }

    /// Unit_CreateWrapper
    pub fn unit_create_wrapper(&mut self, house_id: u8, type_id: u8, destination: u16) -> Option<u16> {
        let r = (self.rng.random_256() & 3) as u16;
        let loc = self.map_find_location_tile(r, house_id);
        let mut tile = Tile32::unpack(loc);
        let orientation = get_direction(tile, Tile32::new(0x2000, 0x2000));

        if self.unit_info[type_id as usize].movement_type == MOVEMENT_WINGER {
            self.validate_strict_if_zero += 1;
            let unit = self.unit_create(INDEX_INVALID, type_id, house_id, tile, orientation);
            self.validate_strict_if_zero -= 1;
            let u = unit?;
            self.um(u).o.flags.by_scenario = true;
            if destination != 0 {
                self.unit_set_destination(u, destination);
            }
            return Some(u);
        }

        self.validate_strict_if_zero += 1;
        let carryall = self.unit_create(INDEX_INVALID, UNIT_CARRYALL, house_id, tile, orientation);
        self.validate_strict_if_zero -= 1;
        let Some(carryall) = carryall else {
            if type_id == UNIT_HARVESTER && self.h(house_id).harvesters_incoming == 0 {
                self.hm(house_id).harvesters_incoming += 1;
            }
            return None;
        };
        if self.house_are_allied(house_id, self.player_house_id) || self.unit_is_type_on_map(house_id, UNIT_CARRYALL) {
            self.um(carryall).o.flags.by_scenario = true;
        }
        tile = Tile32::new(0xFFFF, 0xFFFF);
        self.validate_strict_if_zero += 1;
        let unit = self.unit_create(INDEX_INVALID, type_id, house_id, tile, 0);
        self.validate_strict_if_zero -= 1;
        let Some(unit) = unit else {
            self.unit_remove(carryall);
            if type_id == UNIT_HARVESTER && self.h(house_id).harvesters_incoming == 0 {
                self.hm(house_id).harvesters_incoming += 1;
            }
            return None;
        };
        self.um(carryall).o.flags.in_transport = true;
        self.um(carryall).o.linked_id = (unit & 0xFF) as u8;
        if type_id == UNIT_HARVESTER {
            self.um(unit).amount = 1;
        }
        if destination != 0 {
            self.unit_set_destination(carryall, destination);
        }
        Some(unit)
    }

    /// Unit_FindTargetAround
    pub fn unit_find_target_around(&self, packed: u16) -> u16 {
        const AROUND: [i16; 9] = [0, -1, 1, -64, 64, -65, -63, 65, 63];
        if self.selection_type == SelectionType::Place {
            return packed;
        }
        if self.structure_get_by_packed_tile(packed).is_some() {
            return packed;
        }
        if self.map_get_landscape_type(packed) == LST_BLOOM_FIELD {
            return packed;
        }
        for a in AROUND {
            if let Some(u) = self.unit_get_by_packed_tile(packed.wrapping_add(a as u16)) {
                return self.u(u).o.position.pack();
            }
        }
        packed
    }

    /// Unit_IsTileOccupied
    pub fn unit_is_tile_occupied(&self, u: u16) -> bool {
        let ui = self.ui(u);
        let packed = self.u(u).o.position.pack();
        let speed = self.landscape_info(packed).movement_speed[ui.movement_type as usize];
        if speed == 0 {
            return true;
        }
        if self.u(u).o.type_ == UNIT_SANDWORM || ui.movement_type == MOVEMENT_WINGER {
            return false;
        }
        if let Some(u2) = self.unit_get_by_packed_tile(packed) {
            if u2 != u {
                if self.house_are_allied(self.unit_get_house_id(u2), self.unit_get_house_id(u)) {
                    return true;
                }
                if ui.movement_type != MOVEMENT_TRACKED {
                    return true;
                }
                if self.ui(u2).movement_type != MOVEMENT_FOOT {
                    return true;
                }
            }
        }
        self.structure_get_by_packed_tile(packed).is_some()
    }

    /// Unit_SetSpeed
    pub fn unit_set_speed(&mut self, u: u16, speed: u16) {
        let mut speed = speed;
        {
            let unit = self.um(u);
            unit.speed = 0;
            unit.speed_remainder = 0;
            unit.speed_per_tick = 0;
        }
        if self.u(u).o.type_ == UNIT_HARVESTER {
            speed = (((255 - self.u(u).amount as u32) * speed as u32) / 256) as u16;
        }
        if speed == 0 || speed >= 256 {
            self.um(u).moving_speed = 0;
            return;
        }
        self.um(u).moving_speed = (speed & 0xFF) as u8;
        speed = (self.ui(u).moving_speed_factor as u32 * speed as u32 / 256) as u16;
        if self.ui(u).movement_type != MOVEMENT_WINGER {
            speed = self.adjust_to_game_speed(speed, 1, 255, false);
        }
        let mut speed_per_tick = speed << 4;
        let mut speed = speed >> 4;
        if speed != 0 {
            speed_per_tick = 255;
        } else {
            speed = 1;
        }
        let unit = self.um(u);
        unit.speed = (speed & 0xFF) as u8;
        unit.speed_per_tick = (speed_per_tick & 0xFF) as u8;
    }

    /// Unit_CreateBullet
    pub fn unit_create_bullet(&mut self, position: Tile32, type_: u8, house_id: u8, damage: u16, target: u16) -> Option<u16> {
        if !self.index_is_valid(target) {
            return None;
        }
        let ui = self.unit_info[type_ as usize];
        let tile = self.index_get_tile(target);
        match type_ {
            UNIT_MISSILE_HOUSE | UNIT_MISSILE_ROCKET | UNIT_MISSILE_TURRET | UNIT_MISSILE_DEVIATOR | UNIT_MISSILE_TROOPER => {
                let orientation = get_direction(position, tile);
                let bullet = self.unit_create(INDEX_INVALID, type_, house_id, position, orientation)?;
                self.voice_at_tile(ui.bullet_sound, position);
                {
                    let b = self.um(bullet);
                    b.target_attack = target;
                    b.o.hitpoints = damage;
                    b.current_destination = tile;
                }
                if ui.flags.not_accurate {
                    let d = if self.rng.random_256() & 0xF != 0 {
                        get_distance(position, tile) / 256 + 8
                    } else {
                        self.rng.random_256() as u16 + 8
                    };
                    let dest = self.tile_move_by_random(tile, d, false);
                    self.um(bullet).current_destination = dest;
                }
                self.um(bullet).fire_delay = ui.fire_distance & 0xFF;
                if let Some(t) = self.index_get_unit(target) {
                    if self.ui(t).movement_type == MOVEMENT_WINGER {
                        self.um(bullet).fire_delay <<= 1;
                    }
                }
                if type_ == UNIT_MISSILE_HOUSE || self.u(bullet).o.seen_by_houses & (1 << self.player_house_id) != 0 {
                    return Some(bullet);
                }
                let p = self.u(bullet).o.position;
                self.tile_remove_fog_in_radius(p, 2);
                Some(bullet)
            }
            UNIT_BULLET | UNIT_SONIC_BLAST => {
                let orientation = get_direction(position, tile);
                let t = move_by_direction(move_by_direction(position, 0, 32), orientation as i16, 128);
                let bullet = self.unit_create(INDEX_INVALID, type_, house_id, t, orientation)?;
                if type_ == UNIT_SONIC_BLAST {
                    self.um(bullet).fire_delay = ui.fire_distance & 0xFF;
                }
                {
                    let b = self.um(bullet);
                    b.current_destination = tile;
                    b.o.hitpoints = damage;
                    if damage > 15 {
                        b.o.flags.bullet_is_big = true;
                    }
                }
                if self.u(bullet).o.seen_by_houses & (1 << self.player_house_id) != 0 {
                    return Some(bullet);
                }
                let p = self.u(bullet).o.position;
                self.tile_remove_fog_in_radius(p, 2);
                Some(bullet)
            }
            _ => None,
        }
    }

    /// Unit_DisplayStatusText
    pub fn unit_display_status_text(&mut self, u: u16) {
        use crate::tables::strings::*;
        let ui = *self.ui(u);
        let mut buffer: Vec<u8> = if self.u(u).o.type_ == UNIT_SANDWORM {
            self.string(ui.o.string_id_abbrev)
        } else {
            let house_name = HOUSE_INFO[self.unit_get_house_id(u) as usize].name.as_bytes();
            let mut b = house_name.to_vec();
            b.push(b' ');
            b.extend_from_slice(&self.string(ui.o.string_id_abbrev));
            b
        };
        if self.u(u).o.type_ == UNIT_HARVESTER {
            let mut string_id = STR_IS_D_PERCENT_FULL;
            let unit = self.u(u);
            if unit.action_id == ACTION_HARVEST && unit.amount < 100 {
                let t = self.map_get_landscape_type(unit.o.position.pack());
                if t == LST_SPICE || t == LST_THICK_SPICE {
                    string_id = STR_IS_D_PERCENT_FULL_AND_HARVESTING;
                }
            }
            if unit.action_id == ACTION_MOVE && self.index_get_structure(unit.target_move).is_some() {
                string_id = STR_IS_D_PERCENT_FULL_AND_HEADING_BACK;
            } else if unit.o.script.variables[4] != 0 {
                string_id = STR_IS_D_PERCENT_FULL_AND_AWAITING_PICKUP;
            }
            if unit.amount == 0 {
                string_id += 4;
            }
            let fmt = self.string(string_id);
            let amount = self.u(u).amount as i32;
            buffer.extend(format_c(&fmt, &[Arg::D(amount)]));
        }
        buffer.push(b'.');
        self.display_text(buffer, 2);
    }

    /// Unit_Hide
    pub fn unit_hide(&mut self, u: u16) {
        self.um(u).o.flags.bullet_is_big = true;
        self.unit_update_map(0, u);
        self.um(u).o.flags.bullet_is_big = false;
        self.um(u).o.script.reset(ScriptKind::Unit);
        self.unit_untarget_me(u);
        self.um(u).o.flags.is_not_on_map = true;
        self.unit_house_unit_count_remove(u);
    }

    /// Unit_CallUnitByType
    pub fn unit_call_unit_by_type(&mut self, type_: u8, house_id: u8, target: u16, create_carryall: bool) -> Option<u16> {
        let mut unit = None;
        let mut find = PoolFind::new(house_id, type_ as u16);
        while let Some(u) = self.unit_find(&mut find) {
            if self.u(u).o.linked_id != 0xFF {
                continue;
            }
            if self.u(u).target_move != 0 {
                continue;
            }
            unit = Some(u);
        }
        if create_carryall && unit.is_none() && type_ == UNIT_CARRYALL {
            self.validate_strict_if_zero += 1;
            unit = self.unit_create(INDEX_INVALID, type_, house_id, Tile32::default(), 96);
            self.validate_strict_if_zero -= 1;
            if let Some(u) = unit {
                self.um(u).o.flags.by_scenario = true;
            }
        }
        if let Some(u) = unit {
            self.um(u).target_move = target;
            self.object_script_variable4_set(Some(ObjRef::Unit(u)), target);
        }
        unit
    }

    /// Unit_EnterStructure
    pub fn unit_enter_structure(&mut self, u: u16, s: u16) {
        if self.unit_selected == Some(u) {
            if self.config.dune2_enhanced {
                let p = self.s(s).o.position.pack();
                self.map_set_selection(p);
            } else {
                self.unit_select(None);
            }
        }
        let ui = *self.ui(u);
        let si = *self.si(s);
        if !self.u(u).o.flags.allocated || self.s(s).o.hitpoints == 0 {
            self.unit_remove(u);
            return;
        }
        let seen = self.s(s).o.seen_by_houses;
        self.um(u).o.seen_by_houses |= seen;
        self.unit_hide(u);

        if self.house_are_allied(self.s(s).o.house_id, self.unit_get_house_id(u)) {
            self.structure_set_state(s, if si.o.flags.busy_state_is_incoming { STRUCTURE_STATE_READY } else { STRUCTURE_STATE_BUSY });
            if self.s(s).o.type_ == STRUCTURE_REPAIR {
                let hp = self.u(u).o.hitpoints as u32;
                let count_down = (((ui.o.hitpoints as u32 - hp) * 256 / ui.o.hitpoints as u32) * ((ui.o.build_time as u32) << 6) / 256) as u16;
                self.sm(s).count_down = count_down.max(1);
                let unit = self.um(u);
                unit.o.hitpoints = ui.o.hitpoints;
                unit.o.flags.is_smoking = false;
                unit.sprite_offset = 0;
            }
            let sl = self.s(s).o.linked_id;
            self.um(u).o.linked_id = sl;
            self.sm(s).o.linked_id = (u & 0xFF) as u8;
            return;
        }

        if self.u(u).o.type_ == UNIT_SABOTEUR {
            self.structure_damage(s, 500, 1);
            self.unit_remove(u);
            return;
        }

        if self.s(s).o.hitpoints < si.o.hitpoints / 4 {
            let old_h = self.s(s).o.house_id;
            let new_h = self.unit_get_house_id(u);
            self.sm(s).o.house_id = new_h;
            let b = self.structure_get_structures_built(old_h);
            self.hm(old_h).structures_built = b;
            if self.config.dune2_enhanced {
                self.house_calculate_power_and_credit(old_h);
            }
            let b = self.structure_get_structures_built(new_h);
            self.hm(new_h).structures_built = b;
            if self.s(s).o.linked_id != 0xFF {
                let l = self.s(s).o.linked_id as u16;
                self.um(l).o.house_id = new_h;
            }
            self.house_calculate_power_and_credit(new_h);
            self.structure_update_map(s);
            if self.config.dune2_enhanced {
                self.structure_untarget_me(s);
                self.structure_remove_fog(s);
            }
        } else {
            let d = (self.u(u).o.hitpoints.wrapping_mul(2)).min(self.s(s).o.hitpoints / 2);
            self.structure_damage(s, d, 1);
        }
        self.object_script_variable4_clear(ObjRef::Structure(s));
        self.unit_remove(u);
    }

    /// Unit_FindBestTargetStructure
    fn unit_find_best_target_structure(&mut self, u: u16, mode: u16) -> Option<u16> {
        let position = self.index_get_tile(self.u(u).origin_encoded);
        let distance = self.ui(u).fire_distance << 8;
        let mut best = None;
        let mut best_priority = 0u16;
        let mut find = PoolFind::all();
        while let Some(s) = self.structure_find(&mut find) {
            let t = self.s(s).o.type_;
            if t == STRUCTURE_SLAB_1X1 || t == STRUCTURE_SLAB_2X2 || t == STRUCTURE_WALL {
                continue;
            }
            let layout = self.si(s).layout as usize;
            let cur = add_tile_diff(self.s(s).o.position, crate::tables::STRUCTURE_LAYOUT_TILE_DIFF[layout]);
            if mode != 0 && mode != 4 {
                if mode == 1 {
                    if get_distance(self.u(u).o.position, cur) > distance {
                        continue;
                    }
                } else {
                    if mode != 2 {
                        continue;
                    }
                    if get_distance(position, cur) > distance.wrapping_mul(2) {
                        continue;
                    }
                }
            }
            let priority = self.unit_get_target_structure_priority(u, s);
            if priority >= best_priority {
                best = Some(s);
                best_priority = priority;
            }
        }
        if best_priority == 0 {
            return None;
        }
        best
    }

    /// Unit_GetTileEnterScore
    pub fn unit_get_tile_enter_score(&self, u: u16, packed: u16, orient8: u16) -> i16 {
        let ui = self.ui(u);
        if !self.map_is_valid_position(packed) && ui.movement_type != MOVEMENT_WINGER {
            return 256;
        }
        if let Some(o) = self.unit_get_by_packed_tile(packed) {
            if o != u && self.u(u).o.type_ != UNIT_SANDWORM {
                if self.u(u).o.type_ == UNIT_SABOTEUR && self.u(u).target_move == self.unit_encode(o) {
                    return 0;
                }
                if self.house_are_allied(self.unit_get_house_id(o), self.unit_get_house_id(u)) {
                    return 256;
                }
                if self.ui(o).movement_type != MOVEMENT_FOOT || (ui.movement_type != MOVEMENT_TRACKED && ui.movement_type != MOVEMENT_HARVESTER) {
                    return 256;
                }
            }
        }
        if let Some(s) = self.structure_get_by_packed_tile(packed) {
            let res = self.unit_is_valid_movement_into_structure(u, s);
            if res == 0 {
                return 256;
            }
            return -(res as i16);
        }
        let type_ = self.map_get_landscape_type(packed);
        let mut res: u16 = if self.config.dune2_enhanced {
            (LANDSCAPE_INFO[type_ as usize].movement_speed[ui.movement_type as usize] as u32 * ui.moving_speed_factor as u32 / 256) as u16
        } else {
            LANDSCAPE_INFO[type_ as usize].movement_speed[ui.movement_type as usize] as u16
        };
        if self.u(u).o.type_ == UNIT_SABOTEUR
            && type_ == LST_WALL
            && !self.house_are_allied(self.map[(packed & 0xFFF) as usize].house_id, self.unit_get_house_id(u))
        {
            res = 255;
        }
        if res == 0 {
            return 256;
        }
        if orient8 & 1 != 0 {
            res -= res / 4 + res / 8;
        }
        res ^= 0xFF;
        res as i16
    }

    /// Unit_FindBestTargetEncoded
    pub fn unit_find_best_target_encoded(&mut self, u: u16, mode: u16) -> u16 {
        if mode == 4 {
            if let Some(s) = self.unit_find_best_target_structure(u, mode) {
                return self.structure_encode(s);
            }
            return match self.unit_find_best_target_unit(u, mode) {
                Some(t) => self.unit_encode(t),
                None => 0,
            };
        }
        let target = self.unit_find_best_target_unit(u, mode);
        let s = if self.u(u).o.type_ != UNIT_DEVIATOR { self.unit_find_best_target_structure(u, mode) } else { None };
        match (target, s) {
            (Some(t), Some(s)) => {
                let priority = self.unit_get_target_unit_priority(u, t);
                if self.unit_get_target_structure_priority(u, s) >= priority {
                    self.structure_encode(s)
                } else {
                    self.unit_encode(t)
                }
            }
            (Some(t), None) => self.unit_encode(t),
            (None, Some(s)) => self.structure_encode(s),
            (None, None) => 0,
        }
    }

    /// Unit_RemovePlayer
    pub fn unit_remove_player(&mut self, u: u16) {
        if self.unit_get_house_id(u) != self.player_house_id {
            return;
        }
        if !self.u(u).o.flags.allocated {
            return;
        }
        self.um(u).o.flags.allocated = false;
        self.unit_remove_from_team(u);
        if self.unit_selected != Some(u) {
            return;
        }
        if self.selection_type == SelectionType::Target {
            self.unit_active = None;
            self.active_action = 0xFFFF;
            self.change_selection_type(SelectionType::Structure);
        }
        self.unit_select(None);
    }

    /// Unit_UpdateMap
    pub fn unit_update_map(&mut self, type_: u16, u: u16) {
        const FUNCTIONS: [[u8; 3]; 3] = [[0, 1, 0], [2, 3, 0], [0, 1, 0]];
        let unit = self.u(u);
        if unit.o.flags.is_not_on_map || !unit.o.flags.used {
            return;
        }
        let ui = *self.ui(u);
        if ui.movement_type == MOVEMENT_WINGER {
            if type_ != 0 {
                self.um(u).o.flags.is_dirty = true;
            }
            let pos = self.u(u).o.position;
            self.map_update_around(ui.dimension, pos, Some(u), FUNCTIONS[0][type_ as usize]);
            return;
        }
        let position = self.u(u).o.position;
        let packed = position.pack();
        if self.map[packed as usize].is_unveiled || self.u(u).o.house_id == self.player_house_id {
            let p = self.player_house_id;
            self.unit_house_unit_count_add(u, p);
        } else {
            self.unit_house_unit_count_remove(u);
        }
        if type_ == 1 {
            if self.house_are_allied(self.unit_get_house_id(u), self.player_house_id)
                && !self.map_is_position_unveiled(packed)
                && self.u(u).o.type_ != UNIT_SANDWORM
            {
                self.tile_remove_fog_in_radius(position, 1);
            }
            if self.object_get_by_packed_tile(packed).is_none() {
                let t = &mut self.map[packed as usize];
                t.index = (u + 1) as u8;
                t.has_unit = true;
            }
        }
        if type_ != 0 {
            self.um(u).o.flags.is_dirty = true;
        }
        let unit = self.u(u);
        let mut radius = ui.dimension + 3;
        if unit.o.flags.bullet_is_big || unit.o.flags.is_smoking || (unit.o.type_ == UNIT_HARVESTER && unit.action_id == ACTION_HARVEST) {
            radius = 33;
        }
        let f = FUNCTIONS[1][type_ as usize];
        self.map_update_around(radius, position, Some(u), f);
        if self.u(u).o.type_ != UNIT_HARVESTER {
            return;
        }
        let (tpl, tl) = (self.u(u).target_pre_last, self.u(u).target_last);
        self.map_update_around(radius, tpl, Some(u), f);
        self.map_update_around(radius, tl, Some(u), f);
    }

    /// Unit_RemoveFromTile
    pub fn unit_remove_from_tile(&mut self, u: u16, packed: u16) {
        let t = &self.map[packed as usize];
        if t.has_unit
            && self.unit_get_by_packed_tile(packed) == Some(u)
            && (packed != self.u(u).current_destination.pack() || self.u(u).o.flags.bullet_is_big)
        {
            let t = &mut self.map[packed as usize];
            t.index = 0;
            t.has_unit = false;
        }
    }

    /// Unit_AddToTile
    pub fn unit_add_to_tile(&mut self, u: u16, packed: u16) {
        let h = self.unit_get_house_id(u);
        self.map_unveil_tile(packed, h);
    }

    /// Unit_GetTargetStructurePriority
    pub fn unit_get_target_structure_priority(&self, u: u16, target: u16) -> u16 {
        if self.house_are_allied(self.unit_get_house_id(u), self.s(target).o.house_id) {
            return 0;
        }
        if !self.ai_sees(self.s(target).o.seen_by_houses, self.unit_get_house_id(u)) {
            return 0;
        }
        let si = self.si(target);
        let mut priority = si.o.priority_build.wrapping_add(si.o.priority_target);
        let distance = get_distance_rounded_up(self.u(u).o.position, self.s(target).o.position);
        if distance != 0 {
            priority /= distance;
        }
        priority.min(32000)
    }

    /// Unit_LaunchHouseMissile
    pub fn unit_launch_house_missile(&mut self, packed: u16) {
        let Some(m) = self.unit_house_missile else { return };
        let house = self.u(m).o.house_id;
        let tile = self.tile_move_by_random(Tile32::unpack(packed), 160, false);
        let packed = tile.pack();
        let is_ai = house != self.player_house_id;
        self.unit_free(m);
        self.feedback(0xFFFE);
        let palace = self.h(house).palace_position;
        let t = self.u(m).o.type_;
        let target = self.tile_encode(packed);
        self.unit_create_bullet(palace, t, house, 0x1F4, target);
        self.house_missile_countdown = 0;
        self.unit_house_missile = None;
        if is_ai {
            self.feedback(39);
            return;
        }
        self.change_selection_type(SelectionType::Structure);
    }

    /// Unit_HouseUnitCount_Remove
    pub fn unit_house_unit_count_remove(&mut self, u: u16) {
        if self.u(u).o.seen_by_houses == 0 {
            return;
        }
        for h in self.house_ids() {
            if self.u(u).o.seen_by_houses & (1 << h) == 0 {
                continue;
            }
            if !self.house_are_allied(h, self.unit_get_house_id(u)) {
                self.hm(h).unit_count_enemy = self.h(h).unit_count_enemy.wrapping_sub(1);
            } else {
                self.hm(h).unit_count_allied = self.h(h).unit_count_allied.wrapping_sub(1);
            }
            self.um(u).o.seen_by_houses &= !(1 << h);
        }
        if self.config.dune2_enhanced {
            self.um(u).o.seen_by_houses = 0;
        }
    }

    /// Unit_HouseUnitCount_Add
    pub fn unit_house_unit_count_add(&mut self, u: u16, house_id: u8) {
        let ui = *self.ui(u);
        let p = self.player_house_id;
        let mut bit: u8 = 1 << house_id;
        if house_id == HOUSE_ATREIDES && self.u(u).o.type_ != UNIT_SANDWORM {
            bit |= 1 << HOUSE_FREMEN;
        }
        if self.u(u).o.seen_by_houses & bit != 0 && self.h(house_id).flags.is_ai_active {
            self.um(u).o.seen_by_houses |= bit;
            return;
        }
        if !ui.flags.is_normal_unit && self.u(u).o.type_ != UNIT_SANDWORM {
            return;
        }
        let uh = self.unit_get_house_id(u);
        if self.u(u).o.seen_by_houses & bit == 0 {
            if self.house_are_allied(house_id, uh) {
                self.hm(house_id).unit_count_allied += 1;
            } else {
                self.hm(house_id).unit_count_enemy += 1;
            }
        }
        if ui.movement_type != MOVEMENT_WINGER && !self.house_are_allied(house_id, uh) {
            self.hm(house_id).flags.is_ai_active = true;
            self.hm(uh).flags.is_ai_active = true;
        }
        if house_id == p && self.selection_type != SelectionType::Mentat {
            if self.u(u).o.type_ == UNIT_SANDWORM {
                if self.h(p).timer_sandworm_attack == 0 {
                    if self.music_in_battle == 0 {
                        self.music_in_battle = 1;
                    }
                    self.feedback(37);
                    self.emit(Event::Hint(crate::tables::strings::STR_WARNING_SANDWORMS_SHAIHULUD_ROAM_DUNE_DEVOURING_ANYTHING_ON_THE_SAND, 105));
                    self.hm(p).timer_sandworm_attack = 8;
                }
            } else if !self.house_are_allied(p, uh) {
                if self.h(p).timer_unit_attack == 0 {
                    if self.music_in_battle == 0 {
                        self.music_in_battle = 1;
                    }
                    if self.u(u).o.type_ == UNIT_SABOTEUR {
                        self.feedback(12);
                    } else if self.scenario_id < 3 {
                        let mut find = PoolFind::new(p, STRUCTURE_CONSTRUCTION_YARD as u16);
                        let feedback = match self.structure_find(&mut find) {
                            Some(s) => {
                                let d = get_direction(self.s(s).o.position, self.u(u).o.position) as u8;
                                ((orientation_256_to_8(d) + 1) & 7) as u16 / 2 + 2
                            }
                            None => 1,
                        };
                        self.feedback(feedback);
                    } else {
                        let id = self.u(u).o.house_id as u16 + 6;
                        self.feedback(id);
                    }
                    self.hm(p).timer_unit_attack = 8;
                }
                // Team_Get_ByIndex(unit->team): note the original uses team (not team - 1).
                let t = self.u(u).team as usize;
                if t < TEAM_INDEX_MAX {
                    self.teams[t].script.variables[4] = 1;
                }
            }
        }
        if !self.house_are_allied(house_id, self.u(u).o.house_id) && self.u(u).action_id == ACTION_AMBUSH {
            self.unit_set_action(u, ACTION_HUNT);
        }
        if self.u(u).o.house_id == p || (self.u(u).o.house_id == HOUSE_FREMEN && p == HOUSE_ATREIDES) {
            self.um(u).o.seen_by_houses = 0xFF;
        } else {
            self.um(u).o.seen_by_houses |= bit;
        }
    }
}
