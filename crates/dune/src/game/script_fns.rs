//! Script functions callable from EMC scripts (script/general.c, unit.c, structure.c, team.c).

use super::map::*;
use super::script::Ctx;
use super::*;
use crate::tables::landscape::LANDSCAPE_INFO;
use crate::tables::strings::*;
use crate::tables::{HOUSE_INFO, STRUCTURE_LAYOUT_TILES, STRUCTURE_LAYOUT_TILE_COUNT};

impl Game {
    /// Dispatch the FUNCTION opcode to the function table of the script kind.
    pub fn script_call_function(&mut self, kind: ScriptKind, f: u16, ctx: Ctx) -> u16 {
        match kind {
            ScriptKind::Unit => self.unit_function(f, ctx),
            ScriptKind::Structure => self.structure_function(f, ctx),
            ScriptKind::Team => self.team_function(f, ctx),
            ScriptKind::None => 0,
        }
    }

    fn cur_obj(&self) -> ObjRef {
        self.script_current_object.unwrap_or(ObjRef::Unit(0))
    }
    fn cur_unit(&self) -> u16 {
        self.script_current_unit.unwrap_or(0)
    }
    fn cur_structure(&self) -> u16 {
        self.script_current_structure.unwrap_or(0)
    }
    fn cur_team(&self) -> u16 {
        self.script_current_team.unwrap_or(0)
    }

    fn unit_function(&mut self, f: u16, ctx: Ctx) -> u16 {
        match f {
            0x00 => self.su_get_info(ctx),
            0x01 => self.su_set_action(ctx),
            0x02 => self.sg_display_text(ctx),
            0x03 => self.sg_get_distance_to_tile(ctx),
            0x04 => self.su_start_animation(),
            0x05 => self.su_set_destination(ctx),
            0x06 => self.su_get_orientation(ctx),
            0x07 => self.su_set_orientation(ctx),
            0x08 => self.su_fire(ctx),
            0x09 => self.su_mcv_deploy(),
            0x0A => self.su_set_action_default(),
            0x0B => self.su_blink(),
            0x0C => self.su_calculate_route(ctx),
            0x0D => self.sg_is_enemy(ctx),
            0x0E => self.su_explosion_single(ctx),
            0x0F => self.su_die(),
            0x10 => self.sg_delay(ctx),
            0x11 => self.sg_is_friendly(ctx),
            0x12 => self.su_explosion_multiple(ctx),
            0x13 => self.su_set_sprite(ctx),
            0x14 => self.su_transport_deliver(),
            0x16 => self.su_move_to_target(ctx),
            0x17 => self.sg_random_range(ctx),
            0x18 => self.sg_find_idle(ctx),
            0x19 => self.su_set_destination_direct(ctx),
            0x1A => self.su_stop(),
            0x1B => self.su_set_speed(ctx),
            0x1C => self.su_find_best_target(ctx),
            0x1D => self.su_get_target_priority(ctx),
            0x1E => self.su_move_to_structure(ctx),
            0x1F => self.su_is_in_transport(),
            0x20 => self.su_get_amount(),
            0x21 => self.su_random_soldier(ctx),
            0x22 => self.su_pickup(),
            0x23 => self.su_call_unit_by_type(ctx),
            0x24 => self.su_unknown2552(),
            0x25 => self.su_find_structure(ctx),
            0x26 => self.sg_voice_play(ctx),
            0x27 => self.su_display_destroyed_text(),
            0x28 => self.su_remove_fog(),
            0x29 => self.sg_search_spice(ctx),
            0x2A => self.su_harvest(),
            0x2C => self.sg_get_linked_unit_type(),
            0x2D => self.sg_get_index_type(ctx),
            0x2E => self.sg_decode_index(ctx),
            0x2F => self.su_is_valid_destination(ctx),
            0x30 => self.su_get_random_tile(ctx),
            0x31 => self.su_idle_action(),
            0x32 => self.sg_unit_count(ctx),
            0x33 => self.su_go_to_closest_structure(ctx),
            0x36 => self.su_sandworm_get_best_target(),
            0x37 => self.su_unknown2bd5(),
            0x38 => self.sg_get_orientation(ctx),
            0x3A => self.su_set_target(ctx),
            0x3B => self.sg_unknown0288(ctx),
            0x3C => self.sg_delay_random(ctx),
            0x3D => self.su_rotate(),
            0x3E => self.sg_get_distance_to_object(ctx),
            _ => 0,
        }
    }

    fn structure_function(&mut self, f: u16, ctx: Ctx) -> u16 {
        match f {
            0x00 => self.sg_delay(ctx),
            0x02 => self.ss_unknown0a81(),
            0x03 => self.ss_find_unit_by_type(ctx),
            0x04 => self.ss_set_state(ctx),
            0x05 => self.sg_display_text(ctx),
            0x06 => self.ss_unknown11b9(ctx),
            0x07 => self.ss_unknown0c5a(),
            0x08 => self.ss_find_target_unit(ctx),
            0x09 => self.ss_rotate_turret(ctx),
            0x0A => self.ss_get_direction(ctx),
            0x0B => self.ss_fire(ctx),
            0x0D => self.s(self.cur_structure()).state as u16,
            0x0E => self.ss_voice_play(ctx),
            0x0F => {
                let s = self.cur_structure();
                self.structure_remove_fog(s);
                0
            }
            0x15 => self.ss_refine_spice(),
            0x16 => self.ss_explode(),
            0x17 => self.ss_destroy(),
            _ => 0,
        }
    }

    fn team_function(&mut self, f: u16, ctx: Ctx) -> u16 {
        match f {
            0x00 => self.sg_delay(ctx),
            0x01 => self.st_display_text(ctx),
            0x02 => self.teams[self.cur_team() as usize].members,
            0x03 => self.st_add_closest_unit(),
            0x04 => self.st_get_average_distance(),
            0x05 => self.st_unknown0543(ctx),
            0x06 => self.st_find_best_target(),
            0x07 => self.st_unknown0788(),
            0x08 => self.st_load(ctx),
            0x09 => self.st_load2(),
            0x0A => self.sg_delay_random(ctx),
            0x0B => 0, // DisplayModalMessage: not used in skirmish
            0x0C => self.teams[self.cur_team() as usize].min_members,
            0x0D => self.teams[self.cur_team() as usize].target,
            _ => 0,
        }
    }

    // ---------------------------------------------------------------------------------------
    // General
    // ---------------------------------------------------------------------------------------

    fn sg_delay(&mut self, ctx: Ctx) -> u16 {
        let delay = self.peek(ctx, 1) / 5;
        self.engine(ctx).delay = delay;
        delay
    }

    fn sg_delay_random(&mut self, ctx: Ctx) -> u16 {
        let r = self.rng.random_256() as u32;
        let delay = (r * self.peek(ctx, 1) as u32 / 256) as u16 / 5;
        self.engine(ctx).delay = delay;
        delay
    }

    fn sg_get_distance_to_tile(&mut self, ctx: Ctx) -> u16 {
        let encoded = self.peek(ctx, 1);
        if !self.index_is_valid(encoded) {
            return 0xFFFF;
        }
        let pos = self.obj(self.cur_obj()).position;
        get_distance(pos, self.index_get_tile(encoded))
    }

    fn sg_display_text(&mut self, ctx: Ctx) -> u16 {
        let kind = self.engine(ctx).info;
        let idx = self.peek(ctx, 1);
        let fmt = self.script_info(kind).text_string(idx);
        let a = self.peek(ctx, 2) as i16 as i32;
        let b = self.peek(ctx, 3) as i16 as i32;
        let c = self.peek(ctx, 4) as i16 as i32;
        let text = format_c(&fmt, &[Arg::D(a), Arg::D(b), Arg::D(c)]);
        self.display_text(text, 0);
        0
    }

    fn sg_random_range(&mut self, ctx: Ctx) -> u16 {
        let a = self.peek(ctx, 1);
        let b = self.peek(ctx, 2);
        self.rng.lcg_range(a, b)
    }

    fn sg_get_distance_to_object(&mut self, ctx: Ctx) -> u16 {
        let index = self.peek(ctx, 1);
        if !self.index_is_valid(index) {
            return 0xFFFF;
        }
        self.object_get_distance_to_encoded(self.cur_obj(), index)
    }

    fn sg_unknown0288(&mut self, ctx: Ctx) -> u16 {
        let index = self.peek(ctx, 1);
        if let Some(s) = self.index_get_structure(index) {
            if self.structure_encode(s) != index {
                return 1;
            }
        }
        if self.index_get_object(index).is_none() {
            1
        } else {
            0
        }
    }

    fn sg_get_orientation(&mut self, ctx: Ctx) -> u16 {
        let e = self.peek(ctx, 1);
        match self.index_get_unit(e) {
            Some(u) => self.u(u).orientation[0].current as u8 as u16,
            None => 128,
        }
    }

    fn sg_unit_count(&mut self, ctx: Ctx) -> u16 {
        let t = self.peek(ctx, 1);
        let h = self.obj(self.cur_obj()).house_id;
        self.units_of(h, t).len() as u16
    }

    fn sg_decode_index(&mut self, ctx: Ctx) -> u16 {
        let i = self.peek(ctx, 1);
        if !self.index_is_valid(i) {
            return 0xFFFF;
        }
        Self::index_decode(i)
    }

    fn sg_get_index_type(&mut self, ctx: Ctx) -> u16 {
        let i = self.peek(ctx, 1);
        if !self.index_is_valid(i) {
            return 0xFFFF;
        }
        Self::index_get_type(i)
    }

    fn sg_get_linked_unit_type(&mut self) -> u16 {
        let l = self.obj(self.cur_obj()).linked_id;
        if l == 0xFF {
            return 0xFFFF;
        }
        self.u(l as u16).o.type_ as u16
    }

    fn sg_voice_play(&mut self, ctx: Ctx) -> u16 {
        let pos = self.obj(self.cur_obj()).position;
        let v = self.peek(ctx, 1);
        self.voice_at_tile(v, pos);
        0
    }

    fn sg_search_spice(&mut self, ctx: Ctx) -> u16 {
        let pos = self.obj(self.cur_obj()).position;
        let r = self.peek(ctx, 1);
        let p = self.map_search_spice(pos.pack(), r);
        if p == 0 {
            return 0;
        }
        self.tile_encode(p)
    }

    fn sg_is_friendly(&mut self, ctx: Ctx) -> u16 {
        let index = self.peek(ctx, 1);
        let Some(o) = self.index_get_object(index) else { return 0 };
        let ob = self.obj(o);
        if ob.flags.is_not_on_map || !ob.flags.used {
            return 0;
        }
        if self.sg_is_enemy(ctx) == 0 {
            1
        } else {
            0xFFFF
        }
    }

    fn sg_is_enemy(&mut self, ctx: Ctx) -> u16 {
        let index = self.peek(ctx, 1);
        if !self.index_is_valid(index) {
            return 0;
        }
        let house_id = match self.script_current_unit {
            Some(u) => self.unit_get_house_id(u),
            None => self.obj(self.cur_obj()).house_id,
        };
        match Self::index_get_type(index) {
            IT_UNIT => (self.unit_get_house_id(self.index_get_unit(index).unwrap()) != house_id) as u16,
            IT_STRUCTURE => (self.s(self.index_get_structure(index).unwrap()).o.house_id != house_id) as u16,
            _ => 0,
        }
    }

    fn sg_find_idle(&mut self, ctx: Ctx) -> u16 {
        let index = self.peek(ctx, 1);
        let house_id = self.obj(self.cur_obj()).house_id;
        match Self::index_get_type(index) {
            IT_UNIT | IT_TILE => return 0,
            IT_STRUCTURE => {
                let s = self.index_get_structure(index).unwrap();
                if self.s(s).o.house_id != house_id || self.s(s).state != STRUCTURE_STATE_IDLE {
                    return 0;
                }
                return 1;
            }
            _ => {}
        }
        let mut find = PoolFind::new(house_id, index);
        while let Some(s) = self.structure_find(&mut find) {
            if self.s(s).state != STRUCTURE_STATE_IDLE {
                continue;
            }
            return self.structure_encode(s);
        }
        0
    }

    // ---------------------------------------------------------------------------------------
    // Unit
    // ---------------------------------------------------------------------------------------

    fn su_random_soldier(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        if self.rng.random_256() as u16 >= self.ui(u).o.spawn_chance {
            return 0;
        }
        let pos = self.tile_move_by_random(self.u(u).o.position, 20, true);
        let h = self.u(u).o.house_id;
        let o = self.rng.random_256() as i8;
        let Some(nu) = self.unit_create(INDEX_INVALID, UNIT_SOLDIER, h, pos, o) else { return 0 };
        let d = self.u(u).deviated;
        self.um(nu).deviated = d;
        let a = self.peek(ctx, 1) as u8;
        self.unit_set_action(nu, a);
        1
    }

    fn su_find_best_target(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let m = self.peek(ctx, 1);
        self.unit_find_best_target_encoded(u, m)
    }

    fn su_get_target_priority(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let encoded = self.peek(ctx, 1);
        if let Some(t) = self.index_get_unit(encoded) {
            return self.unit_get_target_unit_priority(u, t);
        }
        match self.index_get_structure(encoded) {
            Some(s) => self.unit_get_target_structure_priority(u, s),
            None => 0,
        }
    }

    fn su_transport_deliver(&mut self) -> u16 {
        let u = self.cur_unit();
        if self.u(u).o.linked_id == 0xFF {
            return 0;
        }
        let tm = self.u(u).target_move;
        if Self::index_get_type(tm) == IT_UNIT {
            return 0;
        }
        if Self::index_get_type(tm) == IT_STRUCTURE {
            let s = self.index_get_structure(tm).unwrap();
            let si = *self.si(s);
            if self.s(s).o.type_ == STRUCTURE_STARPORT {
                let mut ret = 0;
                if self.s(s).state == STRUCTURE_STATE_BUSY {
                    let l = self.u(u).o.linked_id;
                    self.sm(s).o.linked_id = l;
                    let unit = self.um(u);
                    unit.o.linked_id = 0xFF;
                    unit.o.flags.in_transport = false;
                    unit.amount = 0;
                    let p = self.u(u).o.position;
                    self.voice_at_tile(24, p);
                    self.structure_set_state(s, STRUCTURE_STATE_READY);
                    ret = 1;
                }
                self.object_script_variable4_clear(ObjRef::Unit(u));
                self.um(u).target_move = 0;
                return ret;
            }
            let st = self.s(s);
            if (st.state == STRUCTURE_STATE_IDLE || (si.o.flags.busy_state_is_incoming && st.state == STRUCTURE_STATE_BUSY)) && st.o.linked_id == 0xFF {
                let p = self.u(u).o.position;
                self.voice_at_tile(24, p);
                let l = self.u(u).o.linked_id as u16;
                self.unit_enter_structure(l, s);
                self.object_script_variable4_clear(ObjRef::Unit(u));
                let unit = self.um(u);
                unit.target_move = 0;
                unit.o.linked_id = 0xFF;
                unit.o.flags.in_transport = false;
                unit.amount = 0;
                return 1;
            }
            self.object_script_variable4_clear(ObjRef::Unit(u));
            self.um(u).target_move = 0;
            return 0;
        }
        let center = self.u(u).o.position.center();
        if !self.map_is_valid_position(center.pack()) {
            return 0;
        }
        let u2 = self.u(u).o.linked_id as u16;
        if !self.unit_set_position(u2, center) {
            return 0;
        }
        if self.u(u2).o.house_id == self.player_house_id {
            let p = self.u(u).o.position;
            self.voice_at_tile(24, p);
        }
        let o = self.u(u).orientation[0].current;
        self.unit_set_orientation(u2, o, true, 0);
        self.unit_set_orientation(u2, o, true, 1);
        self.unit_set_speed(u2, 0);
        let l2 = self.u(u2).o.linked_id;
        self.um(u).o.linked_id = l2;
        self.um(u2).o.linked_id = 0xFF;
        if self.u(u).o.linked_id != 0xFF {
            return 1;
        }
        self.um(u).o.flags.in_transport = false;
        self.object_script_variable4_clear(ObjRef::Unit(u));
        self.um(u).target_move = 0;
        1
    }

    fn su_pickup(&mut self) -> u16 {
        let u = self.cur_unit();
        if self.u(u).o.linked_id != 0xFF {
            return 0;
        }
        let tm = self.u(u).target_move;
        match Self::index_get_type(tm) {
            IT_STRUCTURE => {
                let s = self.index_get_structure(tm).unwrap();
                if self.s(s).state != STRUCTURE_STATE_READY {
                    self.object_script_variable4_clear(ObjRef::Unit(u));
                    self.um(u).target_move = 0;
                    return 0;
                }
                self.um(u).o.flags.in_transport = true;
                self.object_script_variable4_clear(ObjRef::Unit(u));
                self.um(u).target_move = 0;
                let u2 = self.s(s).o.linked_id as u16;
                self.um(u).o.linked_id = (u2 & 0xFF) as u8;
                let l2 = self.u(u2).o.linked_id;
                self.sm(s).o.linked_id = l2;
                self.um(u2).o.linked_id = 0xFF;
                if self.s(s).o.linked_id == 0xFF {
                    self.structure_set_state(s, STRUCTURE_STATE_IDLE);
                }
                let tl = self.u(u2).target_last;
                if tl.x != 0 || tl.y != 0 {
                    let e = self.tile_encode(tl.pack());
                    self.um(u).target_move = e;
                } else if self.u(u2).o.type_ == UNIT_HARVESTER && self.unit_get_house_id(u2) != self.player_house_id {
                    let sp = self.map_search_spice(self.u(u).o.position.pack(), 20);
                    let e = self.tile_encode(sp);
                    self.um(u).target_move = e;
                }
                1
            }
            IT_UNIT => {
                let u2 = self.index_get_unit(tm).unwrap();
                if !self.u(u2).o.flags.allocated {
                    return 0;
                }
                let hid = self.unit_get_house_id(u);
                let mut s: Option<u16> = None;
                let mut min_distance: i16 = 0;
                let mut find = PoolFind::new(hid, 0xFFFF);
                while let Some(s2) = self.structure_find(&mut find) {
                    let distance = get_distance_rounded_up(self.s(s2).o.position, self.u(u).o.position) as i16;
                    let st2 = self.s(s2);
                    if self.u(u2).o.type_ == UNIT_HARVESTER {
                        if st2.o.type_ != STRUCTURE_REFINERY || st2.state != STRUCTURE_STATE_IDLE || st2.o.script.variables[4] != 0 {
                            continue;
                        }
                        if min_distance != 0 && distance >= min_distance {
                            break;
                        }
                        s = Some(s2);
                        break;
                    }
                    if st2.o.type_ != STRUCTURE_REPAIR || st2.state != STRUCTURE_STATE_IDLE || st2.o.script.variables[4] != 0 {
                        continue;
                    }
                    if min_distance != 0 && distance >= min_distance {
                        continue;
                    }
                    min_distance = distance;
                    s = Some(s2);
                }
                let Some(s) = s else { return 0 };
                if self.unit_selected == Some(u2) {
                    self.unit_select(None);
                }
                self.um(u).o.linked_id = (u2 & 0xFF) as u8;
                self.um(u).o.flags.in_transport = true;
                self.unit_update_map(0, u2);
                self.unit_hide(u2);
                let ue = self.unit_encode(u);
                let se = self.structure_encode(s);
                self.object_script_variable4_link(ue, se);
                let v4 = self.u(u).o.script.variables[4];
                self.um(u).target_move = v4;
                if self.u(u2).o.type_ != UNIT_HARVESTER {
                    return 0;
                }
                if self.map_search_spice(self.u(u2).o.position.pack(), 2) == 0 {
                    let unit = self.um(u2);
                    unit.target_pre_last = Tile32::default();
                    unit.target_last = Tile32::default();
                }
                0
            }
            _ => 0,
        }
    }

    fn su_stop(&mut self) -> u16 {
        let u = self.cur_unit();
        self.unit_set_speed(u, 0);
        0
    }

    fn su_set_speed(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let mut speed = self.peek(ctx, 1).min(255);
        if !self.u(u).o.flags.by_scenario {
            speed = speed * 192 / 256;
        }
        self.unit_set_speed(u, speed);
        self.u(u).speed as u16
    }

    fn su_set_sprite(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let v = (self.peek(ctx, 1) & 0xFF) as u8;
        self.um(u).sprite_offset = (v as i8).wrapping_neg();
        0
    }

    fn su_move_to_target(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        if self.u(u).target_move == 0 {
            return 0;
        }
        let tile = self.index_get_tile(self.u(u).target_move);
        let distance = get_distance(self.u(u).o.position, tile);
        if (distance as i16) < 128 {
            self.unit_set_speed(u, 0);
            let unit = self.um(u);
            let dx = (tile.x.wrapping_sub(unit.o.position.x) as i16).clamp(-16, 16);
            let dy = (tile.y.wrapping_sub(unit.o.position.y) as i16).clamp(-16, 16);
            unit.o.position.x = unit.o.position.x.wrapping_add(dx as u16);
            unit.o.position.y = unit.o.position.y.wrapping_add(dy as u16);
            if (distance as i16) < 32 {
                return 1;
            }
            let e = self.engine(ctx);
            e.delay = 2;
            e.script = e.script.map(|ip| ip - 1);
            return 0;
        }
        let orientation = get_direction(self.u(u).o.position, tile);
        self.unit_set_orientation(u, orientation, false, 0);
        let mut diff = (orientation as i16 - self.u(u).orientation[0].current as i16).abs();
        if diff > 128 {
            diff = 256 - diff;
        }
        let s = ((distance / 8).min(255).max(25) as i32 * (255 - diff as i32) + 128) / 256;
        self.unit_set_speed(u, s as u16);
        let delay = ((distance as i16) / 1024).max(1) as u16;
        if delay != 0 {
            let e = self.engine(ctx);
            e.delay = delay;
            e.script = e.script.map(|ip| ip - 1);
        }
        0
    }

    fn su_die(&mut self) -> u16 {
        let u = self.cur_unit();
        let ui = *self.ui(u);
        self.unit_remove(u);
        if ui.movement_type != MOVEMENT_WINGER {
            let credits = (ui.o.build_credits / 100).max(1);
            if self.u(u).o.house_id == self.player_house_id {
                self.scenario.killed_allied += 1;
                self.scenario.score = self.scenario.score.wrapping_sub(credits);
            } else {
                self.scenario.killed_enemy += 1;
                self.scenario.score = self.scenario.score.wrapping_add(credits);
            }
        }
        self.unit_house_unit_count_remove(u);
        if self.u(u).o.type_ != UNIT_SABOTEUR {
            return 0;
        }
        let p = self.u(u).o.position;
        self.map_make_explosion(EXPLOSION_SABOTEUR_DEATH, p, 300, 0);
        0
    }

    fn su_explosion_single(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let t = self.peek(ctx, 1);
        let p = self.u(u).o.position;
        let hp = self.ui(u).o.hitpoints;
        let e = self.unit_encode(u);
        self.map_make_explosion(t, p, hp, e);
        0
    }

    fn su_explosion_multiple(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let p = self.u(u).o.position;
        let hp = self.rng.lcg_range(25, 50);
        self.map_make_explosion(EXPLOSION_DEATH_HAND, p, hp, 0);
        for _ in 0..7 {
            let r = self.peek(ctx, 1);
            let pos = self.tile_move_by_random(p, r, false);
            let hp = self.rng.lcg_range(75, 150);
            self.map_make_explosion(EXPLOSION_DEATH_HAND, pos, hp, 0);
        }
        0
    }

    fn su_fire(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let target = self.u(u).target_attack;
        if target == 0 || !self.index_is_valid(target) {
            return 0;
        }
        if self.u(u).o.type_ != UNIT_SANDWORM && target == self.tile_encode(self.u(u).o.position.pack()) {
            self.um(u).target_attack = 0;
        }
        if self.u(u).target_attack != target {
            self.unit_set_target(u, target);
            return 0;
        }
        let ui = *self.ui(u);
        let turret = if ui.o.flags.has_turret { 1 } else { 0 };
        if self.u(u).o.type_ != UNIT_SANDWORM && self.u(u).orientation[turret].speed != 0 {
            return 0;
        }
        if Self::index_get_type(target) == IT_TILE && self.object_get_by_packed_tile(self.index_get_packed_tile(target)).is_some() {
            self.unit_set_target(u, target);
        }
        if self.u(u).fire_delay != 0 {
            return 0;
        }
        let distance = self.object_get_distance_to_encoded(ObjRef::Unit(u), target);
        if ((ui.fire_distance << 8) as i16) < distance as i16 {
            return 0;
        }
        let target_is_air = Self::index_get_type(target) == IT_UNIT && self.ui(self.index_get_unit(target).unwrap()).movement_type == MOVEMENT_WINGER;
        if self.u(u).o.type_ != UNIT_SANDWORM && !target_is_air {
            let orientation = get_direction(self.u(u).o.position, self.index_get_tile(target));
            let mut diff = (self.u(u).orientation[turret].current as i16 - orientation as i16).abs();
            if ui.movement_type == MOVEMENT_WINGER {
                diff /= 8;
            }
            if diff >= 8 {
                return 0;
            }
        }
        let mut damage = ui.damage;
        let mut type_id = ui.bullet_type;
        let fire_twice = ui.flags.fires_twice && self.u(u).o.hitpoints > ui.o.hitpoints / 2;
        if (self.u(u).o.type_ == UNIT_TROOPERS || self.u(u).o.type_ == UNIT_TROOPER) && (distance as i16) > 512 {
            type_id = UNIT_MISSILE_TROOPER;
        }
        match type_id {
            UNIT_SANDWORM => {
                self.unit_update_map(0, u);
                if let Some(u2) = self.index_get_unit(target) {
                    self.um(u2).o.script.variables[1] = 0xFFFF;
                    self.unit_remove_player(u2);
                    self.unit_house_unit_count_remove(u2);
                    self.unit_remove(u2);
                }
                let p = self.u(u).o.position;
                self.map_make_explosion(ui.explosion_type, p, 0, 0);
                self.voice_at_tile(63, p);
                self.unit_update_map(1, u);
                self.um(u).amount = self.u(u).amount.wrapping_sub(1);
                self.engine(ctx).delay = 12;
                if (self.u(u).amount as i8) < 1 {
                    self.unit_set_action(u, ACTION_DIE);
                }
            }
            UNIT_MISSILE_TROOPER | UNIT_MISSILE_ROCKET | UNIT_MISSILE_TURRET | UNIT_MISSILE_DEVIATOR | UNIT_BULLET | UNIT_SONIC_BLAST => {
                if type_id == UNIT_MISSILE_TROOPER {
                    damage -= damage / 4;
                }
                let p = self.u(u).o.position;
                let h = self.unit_get_house_id(u);
                let Some(bullet) = self.unit_create_bullet(p, type_id, h, damage, target) else { return 0 };
                let e = self.unit_encode(u);
                self.um(bullet).origin_encoded = e;
                self.voice_at_tile(ui.bullet_sound, p);
                self.unit_deviation_decrease(u, 20);
            }
            _ => {}
        }
        let fd = self.adjust_to_game_speed(ui.fire_delay * 2, 1, 0xFFFF, true);
        self.um(u).fire_delay = fd;
        if fire_twice {
            let f = &mut self.um(u).o.flags;
            f.fire_twice_flip = !f.fire_twice_flip;
            if self.u(u).o.flags.fire_twice_flip {
                let fd = self.adjust_to_game_speed(5, 1, 10, true);
                self.um(u).fire_delay = fd;
            }
        } else {
            self.um(u).o.flags.fire_twice_flip = false;
        }
        let r = (self.rng.random_256() & 1) as u16;
        self.um(u).fire_delay += r;
        1
    }

    fn su_set_orientation(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let o = self.peek(ctx, 1) as u8 as i8;
        self.unit_set_orientation(u, o, false, 0);
        self.u(u).orientation[0].current as u8 as u16
    }

    fn su_rotate(&mut self) -> u16 {
        let u = self.cur_unit();
        let ui = *self.ui(u);
        let cd = self.u(u).current_destination;
        if ui.movement_type != MOVEMENT_WINGER && (cd.x != 0 || cd.y != 0) {
            return 1;
        }
        let index = if ui.o.flags.has_turret { 1 } else { 0 };
        if self.u(u).orientation[index].speed != 0 {
            return 1;
        }
        let current = self.u(u).orientation[index].current;
        if !self.index_is_valid(self.u(u).target_attack) {
            return 0;
        }
        let tile = self.index_get_tile(self.u(u).target_attack);
        let orientation = get_direction(self.u(u).o.position, tile);
        if orientation == current {
            return 0;
        }
        self.unit_set_orientation(u, orientation, false, index);
        1
    }

    fn su_get_orientation(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let encoded = self.peek(ctx, 1);
        if self.index_is_valid(encoded) {
            let tile = self.index_get_tile(encoded);
            return get_direction(self.u(u).o.position, tile) as u8 as u16;
        }
        self.u(u).orientation[0].current as u8 as u16
    }

    fn su_set_destination(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let encoded = self.peek(ctx, 1);
        if encoded == 0 || !self.index_is_valid(encoded) {
            self.um(u).target_move = 0;
            return 0;
        }
        if self.u(u).o.type_ == UNIT_HARVESTER {
            match self.index_get_structure(encoded) {
                None => {
                    let unit = self.um(u);
                    unit.target_move = encoded;
                    unit.route[0] = 0xFF;
                    return 0;
                }
                Some(s) => {
                    if self.s(s).o.script.variables[4] != 0 {
                        return 0;
                    }
                }
            }
        }
        self.unit_set_destination(u, encoded);
        0
    }

    fn su_set_target(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let target = self.peek(ctx, 1);
        if target == 0 || !self.index_is_valid(target) {
            self.um(u).target_attack = 0;
            return 0;
        }
        let tile = self.index_get_tile(target);
        let orientation = get_direction(self.u(u).o.position, tile);
        self.um(u).target_attack = target;
        if !self.ui(u).o.flags.has_turret {
            self.um(u).target_move = target;
            self.unit_set_orientation(u, orientation, false, 0);
        }
        self.unit_set_orientation(u, orientation, false, 1);
        self.u(u).target_attack
    }

    fn su_set_action(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let action = self.peek(ctx, 1) as u8;
        if self.u(u).o.house_id == self.player_house_id && action == ACTION_HARVEST && self.u(u).next_action_id != ACTION_INVALID {
            return 0;
        }
        self.unit_set_action(u, action);
        0
    }

    fn su_set_action_default(&mut self) -> u16 {
        let u = self.cur_unit();
        let a = self.ui(u).o.actions_player[3] as u8;
        self.unit_set_action(u, a);
        0
    }

    fn su_set_destination_direct(&mut self, ctx: Ctx) -> u16 {
        let encoded = self.peek(ctx, 1);
        if !self.index_is_valid(encoded) {
            return 0;
        }
        let u = self.cur_unit();
        let cd = self.u(u).current_destination;
        if (cd.x == 0 && cd.y == 0) || self.ui(u).flags.is_normal_unit {
            let t = self.index_get_tile(encoded);
            self.um(u).current_destination = t;
        }
        let d = get_direction(self.u(u).o.position, self.u(u).current_destination);
        self.unit_set_orientation(u, d, false, 0);
        0
    }

    fn su_get_info(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let ui = *self.ui(u);
        let turret = if ui.o.flags.has_turret { 1 } else { 0 };
        let p = self.peek(ctx, 1);
        let unit = self.u(u).clone();
        match p {
            0x00 => (unit.o.hitpoints as u32 * 256 / ui.o.hitpoints as u32) as u16,
            0x01 => {
                if self.index_is_valid(unit.target_move) {
                    unit.target_move
                } else {
                    0
                }
            }
            0x02 => ui.fire_distance << 8,
            0x03 => unit.o.index,
            0x04 => unit.orientation[0].current as u8 as u16,
            0x05 => unit.target_attack,
            0x06 => {
                if unit.origin_encoded == 0 || unit.o.type_ == UNIT_HARVESTER {
                    self.unit_find_closest_refinery(u);
                }
                self.u(u).origin_encoded
            }
            0x07 => unit.o.type_ as u16,
            0x08 => self.unit_encode(u),
            0x09 => unit.moving_speed as u16,
            0x0A => (unit.orientation[0].target as i16 - unit.orientation[0].current as i16).unsigned_abs(),
            0x0B => (unit.current_destination.x != 0 || unit.current_destination.y != 0) as u16,
            0x0C => (unit.fire_delay == 0) as u16,
            0x0D => ui.flags.explode_on_death as u16,
            0x0E => self.unit_get_house_id(u) as u16,
            0x0F => unit.o.flags.by_scenario as u16,
            0x10 => unit.orientation[turret].current as u8 as u16,
            0x11 => (unit.orientation[turret].target as i16 - unit.orientation[turret].current as i16).unsigned_abs(),
            0x12 => ((ui.movement_type & 0x40) != 0) as u16,
            0x13 => (unit.o.seen_by_houses & (1 << self.player_house_id) != 0) as u16,
            _ => 0,
        }
    }

    fn su_calculate_route(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let encoded = self.peek(ctx, 1);
        let cd = self.u(u).current_destination;
        if cd.x != 0 || cd.y != 0 || !self.index_is_valid(encoded) {
            return 1;
        }
        let packed_src = self.u(u).o.position.pack();
        let packed_dst = self.index_get_packed_tile(encoded);
        if packed_dst == packed_src {
            self.um(u).route[0] = 0xFF;
            self.um(u).target_move = 0;
            return 0;
        }
        if self.u(u).route[0] == 0xFF {
            let res = self.pathfinder(u, packed_src, packed_dst, 40);
            let n = (res.route_size as usize).min(14);
            for i in 0..n {
                self.um(u).route[i] = res.buffer[i];
            }
            if self.u(u).route[0] == 0xFF {
                self.um(u).target_move = 0;
                if self.u(u).o.type_ == UNIT_SANDWORM {
                    self.engine(ctx).delay = 720;
                }
            }
        } else {
            let distance = get_distance_packed(packed_dst, packed_src);
            if distance < 14 {
                self.um(u).route[distance as usize] = 0xFF;
            }
        }
        if self.u(u).route[0] == 0xFF {
            return 1;
        }
        let want = (self.u(u).route[0] as u16 * 32) as u8 as i8;
        if self.u(u).orientation[0].current != want {
            self.unit_set_orientation(u, want, false, 0);
            return 1;
        }
        if !self.unit_start_movement(u) {
            self.um(u).route[0] = 0xFF;
            return 0;
        }
        let unit = self.um(u);
        unit.route.copy_within(1..14, 0);
        unit.route[13] = 0xFF;
        1
    }

    fn su_move_to_structure(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        if self.u(u).o.linked_id != 0xFF {
            let l = self.u(u).o.linked_id as u16;
            if let Some(s) = self.index_get_structure(self.u(l).origin_encoded) {
                if self.s(s).state == STRUCTURE_STATE_IDLE && self.s(s).o.script.variables[4] == 0 {
                    let encoded = self.structure_encode(s);
                    let ue = self.unit_encode(u);
                    self.object_script_variable4_link(ue, encoded);
                    let v = self.u(u).o.script.variables[4];
                    self.um(u).target_move = v;
                    return encoded;
                }
            }
        }
        let h = self.unit_get_house_id(u);
        let t = self.peek(ctx, 1);
        let mut find = PoolFind::new(h, t);
        while let Some(s) = self.structure_find(&mut find) {
            if self.s(s).state != STRUCTURE_STATE_IDLE || self.s(s).o.script.variables[4] != 0 {
                continue;
            }
            let encoded = self.structure_encode(s);
            let ue = self.unit_encode(u);
            self.object_script_variable4_link(ue, encoded);
            self.um(u).target_move = encoded;
            return encoded;
        }
        0
    }

    fn su_get_amount(&mut self) -> u16 {
        let u = self.cur_unit();
        if self.u(u).o.linked_id == 0xFF {
            return self.u(u).amount as u16;
        }
        self.u(self.u(u).o.linked_id as u16).amount as u16
    }

    fn su_is_in_transport(&mut self) -> u16 {
        self.u(self.cur_unit()).o.flags.in_transport as u16
    }

    fn su_start_animation(&mut self) -> u16 {
        let u = self.cur_unit();
        let pos = self.u(u).o.position;
        let position = pos.center().pack();
        self.animation_stop_by_tile(position);
        let mut id: u16 = if LANDSCAPE_INFO[self.map_get_landscape_type(pos.pack()) as usize].is_sand { 0 } else { 1 };
        if self.u(u).o.script.variables[1] == 1 {
            id += 2;
        }
        let h = self.unit_get_house_id(u);
        self.map[position as usize].house_id = h;
        let table = if self.ui(u).display_mode == DISPLAYMODE_INFANTRY_3_FRAMES { AnimTable::UnitScript1 } else { AnimTable::UnitScript2 };
        self.animation_start(Some((table, id)), pos, 0, h, 4);
        1
    }

    fn su_call_unit_by_type(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        if self.u(u).o.script.variables[4] != 0 {
            return self.u(u).o.script.variables[4];
        }
        if !self.ui(u).o.flags.can_be_picked_up || self.u(u).deviated != 0 {
            return 0;
        }
        let encoded = self.unit_encode(u);
        let t = self.peek(ctx, 1) as u8;
        let h = self.unit_get_house_id(u);
        let Some(u2) = self.unit_call_unit_by_type(t, h, encoded, false) else { return 0 };
        let encoded2 = self.unit_encode(u2);
        self.object_script_variable4_link(encoded, encoded2);
        self.um(u2).target_move = encoded;
        encoded2
    }

    fn su_unknown2552(&mut self) -> u16 {
        let u = self.cur_unit();
        let v4 = self.u(u).o.script.variables[4];
        if v4 == 0 {
            return 0;
        }
        let Some(u2) = self.index_get_unit(v4) else { return 0 };
        if self.u(u2).o.type_ != UNIT_CARRYALL {
            return 0;
        }
        self.object_script_variable4_clear(ObjRef::Unit(u));
        self.um(u2).target_move = 0;
        0
    }

    fn su_find_structure(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let h = self.unit_get_house_id(u);
        let t = self.peek(ctx, 1);
        let mut find = PoolFind::new(h, t);
        while let Some(s) = self.structure_find(&mut find) {
            let st = self.s(s);
            if st.state != STRUCTURE_STATE_IDLE || st.o.linked_id != 0xFF || st.o.script.variables[4] != 0 {
                continue;
            }
            return self.structure_encode(s);
        }
        0
    }

    fn su_display_destroyed_text(&mut self) -> u16 {
        let u = self.cur_unit();
        let fmt = self.string(STR_S_S_DESTROYED);
        let house = HOUSE_INFO[self.unit_get_house_id(u) as usize].name.as_bytes().to_vec();
        let name = self.string(self.ui(u).o.string_id_abbrev);
        let text = format_c(&fmt, &[Arg::S(&house), Arg::S(&name)]);
        self.display_text(text, 0);
        0
    }

    fn su_remove_fog(&mut self) -> u16 {
        let u = self.cur_unit();
        self.unit_remove_fog(u);
        0
    }

    fn su_harvest(&mut self) -> u16 {
        let u = self.cur_unit();
        if self.u(u).o.type_ != UNIT_HARVESTER || self.u(u).amount >= 100 {
            return 0;
        }
        let packed = self.u(u).o.position.pack();
        let t = self.map_get_landscape_type(packed);
        if t != LST_SPICE && t != LST_THICK_SPICE {
            return 0;
        }
        let r = self.rng.random_256() & 1;
        let unit = self.um(u);
        unit.amount += r;
        unit.o.flags.in_transport = true;
        if unit.amount > 100 {
            unit.amount = 100;
        }
        if self.rng.random_256() & 0x1F != 0 {
            return 1;
        }
        self.map_change_spice_amount(packed, -1);
        0
    }

    fn su_is_valid_destination(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let encoded = self.peek(ctx, 1);
        let index = Self::index_decode(encoded);
        match Self::index_get_type(encoded) {
            IT_TILE => {
                if !self.map_is_valid_position(index) {
                    return 1;
                }
                if self.u(u).o.linked_id == 0xFF {
                    return 1;
                }
                let u2 = self.u(u).o.linked_id as u16;
                let t = self.index_get_tile(encoded);
                self.um(u2).o.position = t;
                if !self.unit_is_tile_occupied(u2) {
                    return 0;
                }
                self.um(u2).o.position = Tile32::new(0xFFFF, 0xFFFF);
                1
            }
            IT_STRUCTURE => {
                let s = index;
                if self.s(s).o.house_id == self.unit_get_house_id(u) {
                    return 0;
                }
                if self.u(u).o.linked_id == 0xFF {
                    return 1;
                }
                let u2 = self.u(u).o.linked_id as u16;
                (self.unit_is_valid_movement_into_structure(u2, s) != 0) as u16
            }
            _ => 1,
        }
    }

    fn su_get_random_tile(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        if Self::index_get_type(self.peek(ctx, 1)) != IT_TILE {
            return 0;
        }
        let tile = self.tile_move_by_random(self.u(u).o.position, 80, true);
        self.tile_encode(tile.pack())
    }

    fn su_idle_action(&mut self) -> u16 {
        let u = self.cur_unit();
        let random = self.rng.lcg_range(0, 10);
        let mt = self.ui(u).movement_type;
        if mt != MOVEMENT_FOOT && mt != MOVEMENT_TRACKED && mt != MOVEMENT_WHEELED {
            return 0;
        }
        if mt == MOVEMENT_FOOT && random > 8 {
            let r = (self.rng.random_256() & 0x3F) as i8;
            self.um(u).sprite_offset = r;
        }
        if random > 2 {
            return 0;
        }
        let i = if self.rng.random_256() & 1 == 0 { 1 } else { 0 };
        let o = self.rng.random_256() as i8;
        self.unit_set_orientation(u, o, false, i);
        0
    }

    fn su_go_to_closest_structure(&mut self, ctx: Ctx) -> u16 {
        let u = self.cur_unit();
        let h = self.unit_get_house_id(u);
        let t = self.peek(ctx, 1);
        let mut s = None;
        let mut distance_min = 0u16;
        let mut find = PoolFind::new(h, t);
        while let Some(s2) = self.structure_find(&mut find) {
            let st = self.s(s2);
            if st.state != STRUCTURE_STATE_IDLE || st.o.linked_id != 0xFF || st.o.script.variables[4] != 0 {
                continue;
            }
            let d = get_distance_rounded_up(st.o.position, self.u(u).o.position);
            if d >= distance_min && distance_min != 0 {
                continue;
            }
            distance_min = d;
            s = Some(s2);
        }
        let Some(s) = s else { return 0 };
        self.unit_set_action(u, ACTION_MOVE);
        let e = self.structure_encode(s);
        self.unit_set_destination(u, e);
        1
    }

    fn su_mcv_deploy(&mut self) -> u16 {
        let u = self.cur_unit();
        self.unit_update_map(0, u);
        const OFFSETS: [i16; 4] = [0, -1, -64, -65];
        let h = self.unit_get_house_id(u);
        for off in OFFSETS {
            let p = self.u(u).o.position.pack().wrapping_add(off as u16);
            if self.structure_create(INDEX_INVALID, STRUCTURE_CONSTRUCTION_YARD, h, p).is_some() {
                self.unit_remove(u);
                return 1;
            }
        }
        if h == self.player_house_id {
            self.display_string(STR_UNIT_IS_UNABLE_TO_DEPLOY_HERE, 0);
        }
        self.unit_update_map(1, u);
        0
    }

    fn su_sandworm_get_best_target(&mut self) -> u16 {
        let u = self.cur_unit();
        match self.unit_sandworm_find_best_target(u) {
            Some(t) => self.unit_encode(t),
            None => 0,
        }
    }

    fn su_unknown2bd5(&mut self) -> u16 {
        let u = self.cur_unit();
        let v4 = self.u(u).o.script.variables[4];
        let ue = self.unit_encode(u);
        match Self::index_get_type(v4) {
            IT_UNIT => {
                let u2 = self.index_get_unit(v4).unwrap();
                if ue == self.u(u2).o.script.variables[4] && self.u(u2).o.house_id == self.u(u).o.house_id {
                    return 1;
                }
                self.um(u2).target_move = 0;
            }
            IT_STRUCTURE => {
                let s = self.index_get_structure(v4).unwrap();
                if ue == self.s(s).o.script.variables[4] && self.s(s).o.house_id == self.u(u).o.house_id {
                    return 1;
                }
            }
            _ => {}
        }
        self.object_script_variable4_clear(ObjRef::Unit(u));
        0
    }

    fn su_blink(&mut self) -> u16 {
        let u = self.cur_unit();
        self.um(u).blink_counter = 32;
        0
    }

    // ---------------------------------------------------------------------------------------
    // Structure
    // ---------------------------------------------------------------------------------------

    fn ss_set_state(&mut self, ctx: Ctx) -> u16 {
        let s = self.cur_structure();
        let mut state = self.peek(ctx, 1) as i16;
        if state == STRUCTURE_STATE_DETECT {
            state = if self.s(s).o.linked_id == 0xFF {
                STRUCTURE_STATE_IDLE
            } else if self.s(s).count_down == 0 {
                STRUCTURE_STATE_READY
            } else {
                STRUCTURE_STATE_BUSY
            };
        }
        self.structure_set_state(s, state);
        0
    }

    fn ss_refine_spice(&mut self) -> u16 {
        let s = self.cur_structure();
        if self.s(s).o.linked_id == 0xFF {
            self.structure_set_state(s, STRUCTURE_STATE_IDLE);
            return 0;
        }
        let u = self.s(s).o.linked_id as u16;
        let si = *self.si(s);
        let mut harvester_step = ((self.s(s).o.hitpoints as u32 * 256 / si.o.hitpoints as u32) * 3 / 256) as u16;
        let amount = self.u(u).amount as u16;
        if amount < harvester_step {
            harvester_step = amount;
        }
        if amount != 0 && harvester_step < 1 {
            harvester_step = 1;
        }
        if harvester_step == 0 {
            return 0;
        }
        let mut credits_step: u16 = 7;
        if self.u(u).o.house_id != self.player_house_id {
            credits_step = credits_step.wrapping_add((self.rng.random_256() % 4) as u16).wrapping_sub(1);
        }
        credits_step = credits_step.wrapping_mul(harvester_step);
        let hid = self.s(s).o.house_id;
        if self.house_are_allied(self.player_house_id, hid) {
            self.scenario.harvested_allied = self.scenario.harvested_allied.saturating_add(credits_step).min(65000);
        } else {
            self.scenario.harvested_enemy = self.scenario.harvested_enemy.saturating_add(credits_step).min(65000);
        }
        self.hm(hid).credits = self.h(hid).credits.wrapping_add(credits_step);
        self.um(u).amount -= harvester_step as u8;
        if self.u(u).amount == 0 {
            self.um(u).o.flags.in_transport = false;
        }
        self.sm(s).o.script.delay = 6;
        1
    }

    fn ss_unknown0a81(&mut self) -> u16 {
        let s = self.cur_structure();
        let structure_index = self.structure_encode(s);
        let v4 = self.s(s).o.script.variables[4];
        if let Some(u) = self.index_get_unit(v4) {
            if structure_index == self.u(u).o.script.variables[4] {
                return v4;
            }
            self.object_script_variable4_clear(ObjRef::Unit(u));
        }
        self.object_script_variable4_clear(ObjRef::Structure(s));
        0
    }

    fn ss_find_unit_by_type(&mut self, ctx: Ctx) -> u16 {
        let s = self.cur_structure();
        if self.s(s).state != STRUCTURE_STATE_READY || self.s(s).o.linked_id == 0xFF {
            return IT_NONE;
        }
        let type_ = self.peek(ctx, 1) as u8;
        let position = self.structure_find_free_position(s, false);
        let u = self.s(s).o.linked_id as u16;
        let hid = self.s(s).o.house_id;
        let tl = self.u(u).target_last;
        if self.player_house_id == hid && self.u(u).o.type_ == UNIT_HARVESTER && tl.x == 0 && tl.y == 0 && position != 0 {
            return IT_NONE;
        }
        let se = self.structure_encode(s);
        let Some(carryall) = self.unit_call_unit_by_type(type_, hid, se, position == 0) else { return IT_NONE };
        let ce = self.unit_encode(carryall);
        self.object_script_variable4_set(Some(ObjRef::Structure(s)), ce);
        ce
    }

    fn ss_unknown0c5a(&mut self) -> u16 {
        let s = self.cur_structure();
        if self.s(s).o.linked_id == 0xFF {
            return 0;
        }
        let u = self.s(s).o.linked_id as u16;
        let spos = self.s(s).o.position;
        if self.ui(u).movement_type == MOVEMENT_WINGER && self.unit_set_position(u, spos) {
            let l = self.u(u).o.linked_id;
            self.sm(s).o.linked_id = l;
            self.um(u).o.linked_id = 0xFF;
            if self.s(s).o.linked_id == 0xFF {
                self.structure_set_state(s, STRUCTURE_STATE_IDLE);
            }
            self.object_script_variable4_clear(ObjRef::Structure(s));
            if self.s(s).o.house_id == self.player_house_id {
                self.feedback(self.player_house_id as u16 + 49);
            }
            return 1;
        }
        let harv = self.u(u).o.type_ == UNIT_HARVESTER;
        let position = self.structure_find_free_position(s, harv);
        if position == 0 {
            return 0;
        }
        let seen = self.s(s).o.seen_by_houses;
        self.um(u).o.seen_by_houses |= seen;
        let tile = Tile32::unpack(position).center();
        if !self.unit_set_position(u, tile) {
            return 0;
        }
        let l = self.u(u).o.linked_id;
        self.sm(s).o.linked_id = l;
        self.um(u).o.linked_id = 0xFF;
        let d = (get_direction(spos, self.u(u).o.position) as u8 & 0xE0) as i8;
        self.unit_set_orientation(u, d, true, 0);
        let c = self.u(u).orientation[0].current;
        self.unit_set_orientation(u, c, true, 1);
        if self.u(u).o.house_id == self.player_house_id && harv {
            self.emit(Event::Hint(STR_SEARCH_FOR_SPICE_FIELDS_TO_HARVEST, 0x6A));
        }
        if self.s(s).o.linked_id == 0xFF {
            self.structure_set_state(s, STRUCTURE_STATE_IDLE);
        }
        self.object_script_variable4_clear(ObjRef::Structure(s));
        if self.s(s).o.house_id != self.player_house_id || self.s(s).o.type_ == STRUCTURE_REPAIR {
            return 1;
        }
        self.feedback(self.player_house_id as u16 + if harv { 68 } else { 30 });
        1
    }

    fn ss_find_target_unit(&mut self, ctx: Ctx) -> u16 {
        let s = self.cur_structure();
        let target_range = self.peek(ctx, 1) as u32;
        let mut distance_current: u32 = 32000;
        let mut best = None;
        let position = if self.config.dune2_enhanced { self.s(s).o.position.center() } else { self.s(s).o.position };
        let hid = self.s(s).o.house_id;
        let mut find = PoolFind::all();
        while let Some(uf) = self.unit_find(&mut find) {
            if self.house_are_allied(hid, self.unit_get_house_id(uf)) {
                continue;
            }
            let t = self.u(uf).o.type_;
            if t != UNIT_ORNITHOPTER && !self.ai_sees(self.u(uf).o.seen_by_houses, hid) {
                continue;
            }
            let distance = get_distance(self.u(uf).o.position, position) as u32;
            if distance >= distance_current {
                continue;
            }
            if self.config.dune2_enhanced {
                if t == UNIT_ORNITHOPTER {
                    if distance > target_range * 3 {
                        continue;
                    }
                } else if distance > target_range {
                    continue;
                }
            } else if t == UNIT_ORNITHOPTER {
                if distance >= target_range * 3 {
                    continue;
                }
            } else if distance >= target_range {
                continue;
            }
            if self.config.dune2_enhanced {
                distance_current = distance;
            }
            best = Some(uf);
        }
        match best {
            Some(u) => self.unit_encode(u),
            None => IT_NONE,
        }
    }

    fn ss_rotate_turret(&mut self, ctx: Ctx) -> u16 {
        let encoded = self.peek(ctx, 1);
        if encoded == 0 {
            return 0;
        }
        let s = self.cur_structure();
        let look_at = self.index_get_tile(encoded);
        let packed = self.s(s).o.position.pack();
        let group = if self.s(s).o.type_ == STRUCTURE_ROCKET_TURRET { 24 } else { 23 };
        let base_tile_id = self.icon_map[self.icon_map[group] as usize + 2];
        let rotation = self.map[packed as usize].ground_tile_id as i16 - base_tile_id as i16;
        if !(0..=7).contains(&rotation) {
            return 1;
        }
        let needed = orientation_256_to_8(get_direction(self.s(s).o.position, look_at) as u8) as i16;
        if needed == rotation {
            return 0;
        }
        let mut diff = needed - rotation;
        if diff < 0 {
            diff += 8;
        }
        let mut rotation = if diff < 4 { rotation + 1 } else { rotation - 1 };
        rotation &= 0x7;
        self.map[packed as usize].ground_tile_id = base_tile_id + rotation as u16;
        self.sm(s).rotation_sprite_diff = rotation as u16;
        1
    }

    fn ss_get_direction(&mut self, ctx: Ctx) -> u16 {
        let s = self.cur_structure();
        let encoded = self.peek(ctx, 1);
        if !self.index_is_valid(encoded) {
            return self.s(s).rotation_sprite_diff << 5;
        }
        let tile = self.index_get_tile(encoded);
        (orientation_256_to_8(get_direction(self.s(s).o.position, tile) as u8) as u16) << 5
    }

    fn ss_unknown11b9(&mut self, ctx: Ctx) -> u16 {
        let encoded = self.peek(ctx, 1);
        if !self.index_is_valid(encoded) || Self::index_get_type(encoded) != IT_UNIT {
            return 0;
        }
        let Some(u) = self.index_get_unit(encoded) else { return 0 };
        self.object_script_variable4_clear(ObjRef::Unit(u));
        self.um(u).target_move = 0;
        0
    }

    fn ss_voice_play(&mut self, ctx: Ctx) -> u16 {
        let s = self.cur_structure();
        if self.s(s).o.house_id != self.player_house_id {
            return 0;
        }
        let v = self.peek(ctx, 1);
        let p = self.s(s).o.position;
        self.voice_at_tile(v, p);
        0
    }

    fn ss_fire(&mut self, ctx: Ctx) -> u16 {
        let s = self.cur_structure();
        let target = self.engine(ctx).variables[2];
        if target == 0 {
            return 0;
        }
        let (type_, damage, fire_delay) = if self.s(s).o.type_ == STRUCTURE_ROCKET_TURRET
            && get_distance(self.index_get_tile(target), self.s(s).o.position) >= 0x300
        {
            (UNIT_MISSILE_TURRET, 30, self.adjust_to_game_speed(self.unit_info[UNIT_LAUNCHER as usize].fire_delay, 1, 0xFFFF, true))
        } else {
            (UNIT_BULLET, 20, self.adjust_to_game_speed(self.unit_info[UNIT_TANK as usize].fire_delay, 1, 0xFFFF, true))
        };
        let p = self.s(s).o.position;
        let position = Tile32::new(p.x.wrapping_add(0x80), p.y.wrapping_add(0x80));
        let h = self.s(s).o.house_id;
        let Some(u) = self.unit_create_bullet(position, type_, h, damage, target) else { return 0 };
        let e = self.structure_encode(s);
        self.um(u).origin_encoded = e;
        fire_delay
    }

    fn ss_explode(&mut self) -> u16 {
        let s = self.cur_structure();
        let layout = self.si(s).layout as usize;
        let position = self.s(s).o.position.pack();
        for i in 0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
            let tile = Tile32::unpack(position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][i]));
            self.map_make_explosion(EXPLOSION_STRUCTURE, tile, 0, 0);
        }
        0
    }

    fn ss_destroy(&mut self) -> u16 {
        let s = self.cur_structure();
        let si = *self.si(s);
        let layout = si.layout as usize;
        let position = self.s(s).o.position.pack();
        self.structure_remove(s);
        let hid = self.s(s).o.house_id;
        for i in 0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize {
            let tile = Tile32::unpack(position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][i]));
            if si.o.spawn_chance < self.rng.random_256() as u16 {
                continue;
            }
            let o = self.rng.random_256() as i8;
            let Some(u) = self.unit_create(INDEX_INVALID, UNIT_SOLDIER, hid, tile, o) else { continue };
            let r = (self.rng.random_256() & 3) as u32;
            self.um(u).o.hitpoints = (self.unit_info[UNIT_SOLDIER as usize].o.hitpoints as u32 * r / 256) as u16;
            if hid != self.player_house_id {
                self.unit_set_action(u, ACTION_ATTACK);
                continue;
            }
            self.unit_set_action(u, ACTION_MOVE);
            let t = self.tile_move_by_random(self.u(u).o.position, 32, true);
            let e = self.tile_encode(t.pack());
            self.um(u).target_move = e;
        }
        if hid != self.player_house_id {
            return 0;
        }
        let mut text = HOUSE_INFO[hid as usize].name.as_bytes().to_vec();
        text.push(b' ');
        text.extend(self.string(si.o.string_id_full));
        text.push(b' ');
        text.extend(self.string(STR_IS_DESTROYED));
        self.display_text(text, 0);
        0
    }

    // ---------------------------------------------------------------------------------------
    // Team
    // ---------------------------------------------------------------------------------------

    fn st_display_text(&mut self, ctx: Ctx) -> u16 {
        let t = self.cur_team();
        if self.teams[t as usize].house_id == self.player_house_id {
            return 0;
        }
        self.sg_display_text(ctx)
    }

    fn st_add_closest_unit(&mut self) -> u16 {
        let t = self.cur_team() as usize;
        if self.teams[t].members >= self.teams[t].max_members {
            return 0;
        }
        let mut closest = None;
        let mut closest2 = None;
        let mut min_distance = 0u16;
        let mut min_distance2 = 0u16;
        let hid = self.teams[t].house_id;
        let mut find = PoolFind::new(hid, 0xFFFF);
        while let Some(u) = self.unit_find(&mut find) {
            let unit = self.u(u);
            if !unit.o.flags.by_scenario || unit.o.type_ == UNIT_SABOTEUR {
                continue;
            }
            if self.ui(u).movement_type != self.teams[t].movement_type {
                continue;
            }
            if unit.team == 0 {
                let d = get_distance(self.teams[t].position, unit.o.position);
                if d >= min_distance && min_distance != 0 {
                    continue;
                }
                min_distance = d;
                closest = Some(u);
                continue;
            }
            let t2 = &self.teams[unit.team as usize - 1];
            if t2.members > t2.min_members {
                continue;
            }
            let d = get_distance(self.teams[t].position, unit.o.position);
            if d >= min_distance2 && min_distance2 != 0 {
                continue;
            }
            min_distance2 = d;
            closest2 = Some(u);
        }
        let Some(c) = closest.or(closest2) else { return 0 };
        self.unit_remove_from_team(c);
        self.unit_add_to_team(c, t as u16)
    }

    fn st_get_average_distance(&mut self) -> u16 {
        let t = self.cur_team() as usize;
        let hid = self.teams[t].house_id;
        let members: Vec<u16> = self.units_of(hid, 0xFFFF).into_iter().filter(|&u| self.u(u).team as i32 - 1 == t as i32).collect();
        let count = members.len() as u16;
        if count == 0 {
            return 0;
        }
        let mut ax: u16 = 0;
        let mut ay: u16 = 0;
        for &u in &members {
            ax = ax.wrapping_add(self.u(u).o.position.pos_x());
            ay = ay.wrapping_add(self.u(u).o.position.pos_y());
        }
        ax /= count;
        ay /= count;
        self.teams[t].position = Tile32::make_xy(ax, ay);
        let mut distance: u16 = 0;
        for &u in &members {
            distance = distance.wrapping_add(get_distance_rounded_up(self.u(u).o.position, self.teams[t].position));
        }
        distance /= count;
        let team = &self.teams[t];
        if team.target == 0 || team.target_tile == 0 {
            return distance;
        }
        if get_distance_packed(crate::tile::pack_xy(ax, ay), self.index_get_packed_tile(team.target)) <= 10 {
            self.teams[t].target_tile = 2;
        }
        distance
    }

    fn st_unknown0543(&mut self, ctx: Ctx) -> u16 {
        let t = self.cur_team() as usize;
        let distance = self.peek(ctx, 1);
        let hid = self.teams[t].house_id;
        let mut count = 0u16;
        let mut find = PoolFind::new(hid, 0xFFFF);
        while let Some(u) = self.unit_find(&mut find) {
            if self.u(u).team as i32 - 1 != t as i32 {
                continue;
            }
            let tm = self.u(u).target_move;
            let tile = self.index_get_tile(tm);
            let tpos = self.teams[t].position;
            let upos = self.u(u).o.position;
            let d_unit_team = get_distance_rounded_up(upos, tpos);
            let (d_unit_dest, d_team_dest) = if tm != 0 { (get_distance_rounded_up(upos, tile), get_distance_rounded_up(tpos, tile)) } else { (64, 64) };
            if (d_unit_dest < d_team_dest && distance.wrapping_add(2) < d_unit_team) || (d_unit_dest >= d_team_dest && d_unit_team > distance) {
                self.unit_set_action(u, ACTION_MOVE);
                let tile = self.tile_move_by_random(tpos, distance << 4, true);
                let e = self.tile_encode(tile.pack());
                self.unit_set_destination(u, e);
                count += 1;
                continue;
            }
            self.unit_set_action(u, ACTION_GUARD);
        }
        count
    }

    fn st_find_best_target(&mut self) -> u16 {
        let t = self.cur_team() as usize;
        let hid = self.teams[t].house_id;
        let mut find = PoolFind::new(hid, 0xFFFF);
        while let Some(u) = self.unit_find(&mut find) {
            if self.u(u).team as i32 - 1 != t as i32 {
                continue;
            }
            let mode = if self.teams[t].action == TEAM_ACTION_KAMIKAZE { 4 } else { 0 };
            let target = self.unit_find_best_target_encoded(u, mode);
            if target == 0 {
                continue;
            }
            if self.teams[t].target == target {
                return target;
            }
            self.teams[t].target = target;
            let from = self.u(u).o.position.pack();
            let to = self.index_get_packed_tile(target);
            let tt = self.tile_get_tile_in_direction_of(from, to);
            self.teams[t].target_tile = tt;
            return target;
        }
        0
    }

    fn st_load(&mut self, ctx: Ctx) -> u16 {
        let t = self.cur_team() as usize;
        let type_ = self.peek(ctx, 1);
        if self.teams[t].action == type_ {
            return 0;
        }
        self.teams[t].action = type_;
        self.teams[t].script.reset(ScriptKind::Team);
        self.script_load(Ctx::Team(t as u16), (type_ & 0xFF) as u8);
        0
    }

    fn st_load2(&mut self) -> u16 {
        let t = self.cur_team() as usize;
        let type_ = self.teams[t].action_start;
        if self.teams[t].action == type_ {
            return 0;
        }
        self.teams[t].action = type_;
        self.teams[t].script.reset(ScriptKind::Team);
        self.script_load(Ctx::Team(t as u16), (type_ & 0xFF) as u8);
        0
    }

    fn st_unknown0788(&mut self) -> u16 {
        let t = self.cur_team() as usize;
        if self.teams[t].target == 0 {
            return 0;
        }
        let tile = self.index_get_tile(self.teams[t].target);
        let hid = self.teams[t].house_id;
        let mut find = PoolFind::new(hid, 0xFFFF);
        while let Some(u) = self.unit_find(&mut find) {
            if self.u(u).team as i32 - 1 != t as i32 {
                continue;
            }
            let target = self.teams[t].target;
            if target == 0 {
                self.unit_set_action(u, ACTION_GUARD);
                continue;
            }
            let distance = self.ui(u).fire_distance << 8;
            if self.u(u).action_id == ACTION_ATTACK && self.u(u).target_attack == target {
                if self.u(u).target_move != 0 {
                    continue;
                }
                if get_distance(self.u(u).o.position, tile) >= distance {
                    continue;
                }
            }
            if self.u(u).action_id != ACTION_ATTACK {
                self.unit_set_action(u, ACTION_ATTACK);
            }
            let mut orientation = (get_direction(tile, self.u(u).o.position) as u8 & 0xC0) as i16 + self.rng.lcg_range(0, 127) as i16;
            if orientation < 0 {
                orientation += 256;
            }
            let packed = move_by_direction(tile, orientation, distance).pack();
            let dest = if self.object_get_by_packed_tile(packed).is_none() { self.tile_encode(packed) } else { self.tile_encode(tile.pack()) };
            self.unit_set_destination(u, dest);
            self.unit_set_target(u, target);
        }
        0
    }
}
