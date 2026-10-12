//! Houses (house.c).

use super::*;
use crate::tables::strings::*;
use crate::tables::HOUSE_INFO;

impl Game {
    /// GameLoop_House
    pub fn game_loop_house(&mut self) {
        let now = self.timer_game;
        let mut tick_house = false;
        let mut tick_power_maintenance = false;
        let mut tick_starport = false;
        let mut tick_reinforcement = false;
        let mut tick_missile_countdown = false;
        let mut tick_starport_availability = false;

        if self.ticks.house_house <= now {
            tick_house = true;
            self.ticks.house_house = now + 900;
        }
        if self.ticks.house_power_maintenance <= now {
            tick_power_maintenance = true;
            self.ticks.house_power_maintenance = now + 10800;
        }
        if self.ticks.house_starport <= now {
            tick_starport = true;
            self.ticks.house_starport = now + 180;
        }
        if self.ticks.house_reinforcement <= now {
            tick_reinforcement = true;
            self.ticks.house_reinforcement = now + 600;
        }
        if self.ticks.house_missile_countdown <= now {
            tick_missile_countdown = true;
            self.ticks.house_missile_countdown = now + 60;
        }
        if self.ticks.house_starport_availability <= now {
            tick_starport_availability = true;
            self.ticks.house_starport_availability = now + 1800;
        }

        if tick_missile_countdown && self.house_missile_countdown != 0 {
            self.house_missile_countdown -= 1;
            self.feedback(self.house_missile_countdown + 41);
            if self.house_missile_countdown == 0 {
                let p = self.player_house_id;
                let t = self.map_find_location_tile(4, p);
                self.unit_launch_house_missile(t);
            }
        }

        if tick_starport_availability {
            let type_ = self.rng.lcg_range(0, UNIT_MAX as u16 - 1) as usize;
            let a = self.starport_available[type_];
            if a != 0 && a < 10 {
                self.starport_available[type_] = if a == -1 { 1 } else { a + 1 };
            }
        }

        if tick_reinforcement {
            self.house_tick_reinforcements();
        }

        let mut find = PoolFind::all();
        while let Some(h) = self.house_find(&mut find) {
            if tick_house {
                if h != self.player_house_id {
                    let cap = self.skirmish_ai_credit_cap(h);
                    if cap < self.h(h).credits {
                        self.hm(h).credits = cap;
                    }
                } else {
                    let max_credits = self.h(h).credits_storage.max(self.player_credits_no_silo);
                    if self.h(h).credits > max_credits {
                        self.hm(h).credits = max_credits;
                        self.display_string(STR_INSUFFICIENT_SPICE_STORAGE_AVAILABLE_SPICE_IS_LOST, 1);
                    }
                }
                if h == self.player_house_id {
                    if self.h(h).credits_storage > self.player_credits_no_silo {
                        self.player_credits_no_silo = 0;
                    }
                    if self.player_credits_no_silo == 0 && self.campaign_id > 1 && self.h(h).credits != 0 {
                        let hh = self.h(h);
                        if hh.credits_storage != 0 && (hh.credits as u32 * 256 / hh.credits_storage as u32) > 200 {
                            self.display_string(STR_SPICE_STORAGE_CAPACITY_LOW_BUILD_SILOS, 0);
                        }
                    }
                    if self.h(h).credits < 100 && self.player_credits_no_silo != 0 {
                        self.display_string(STR_CREDITS_ARE_LOW_HARVEST_SPICE_FOR_MORE_CREDITS, 0);
                    }
                }
            }

            if tick_house {
                self.house_ensure_harvester_available(h);
            }

            if tick_starport && self.h(h).starport_linked_id != INDEX_INVALID {
                let left = self.h(h).starport_time_left.wrapping_sub(1);
                self.hm(h).starport_time_left = if (left as i16) < 0 { 0 } else { left };
                if self.h(h).starport_time_left == 0 {
                    let mut u = None;
                    let si = self.structure_index;
                    if self.s(si).o.type_ == STRUCTURE_STARPORT && self.s(si).o.house_id == h {
                        let e = self.structure_encode(si);
                        u = self.unit_create_wrapper(h, UNIT_FRIGATE, e);
                    } else {
                        let mut find2 = PoolFind::new(h, STRUCTURE_STARPORT as u16);
                        while let Some(s) = self.structure_find(&mut find2) {
                            if self.s(s).o.linked_id != 0xFF {
                                continue;
                            }
                            let e = self.structure_encode(s);
                            u = self.unit_create_wrapper(h, UNIT_FRIGATE, e);
                            break;
                        }
                    }
                    if let Some(u) = u {
                        let link = self.h(h).starport_linked_id;
                        self.um(u).o.linked_id = link as u8;
                        self.hm(h).starport_linked_id = INDEX_INVALID;
                        self.um(u).o.flags.in_transport = true;
                        if h == self.player_house_id {
                            self.feedback(38);
                        }
                    }
                    self.hm(h).starport_time_left = if u.is_some() { HOUSE_INFO[h as usize].starport_delivery_time } else { 1 };
                }
            }

            if tick_house {
                self.house_calculate_power_and_credit(h);
                self.structure_calculate_hitpoints_max(h);
                let hh = self.hm(h);
                if hh.timer_unit_attack != 0 {
                    hh.timer_unit_attack -= 1;
                }
                if hh.timer_sandworm_attack != 0 {
                    hh.timer_sandworm_attack -= 1;
                }
                if hh.timer_structure_attack != 0 {
                    hh.timer_structure_attack -= 1;
                }
                if self.h(h).harvesters_incoming > 0 && self.unit_create_wrapper(h, UNIT_HARVESTER, 0).is_some() {
                    self.hm(h).harvesters_incoming -= 1;
                }
            }

            if tick_power_maintenance {
                let cost = (self.h(h).power_usage / 32) + 1;
                let c = self.h(h).credits;
                self.hm(h).credits -= c.min(cost);
            }
        }
    }

    fn house_tick_reinforcements(&mut self) {
        let mut nu: Option<u16> = None;
        for i in 0..16 {
            let r = self.scenario.reinforcement[i];
            if r.unit_id == INDEX_INVALID || r.time_left == 0 {
                continue;
            }
            self.scenario.reinforcement[i].time_left -= 1;
            if self.scenario.reinforcement[i].time_left != 0 {
                continue;
            }
            let u = r.unit_id;
            let hid = self.u(u).o.house_id;
            let location_id = r.location_id;
            let mut deployed = false;
            if location_id >= 4 {
                if nu.is_none() {
                    let l = (self.rng.random_256() & 3) as u16;
                    let p = self.map_find_location_tile(l, hid);
                    nu = self.unit_create(INDEX_INVALID, UNIT_CARRYALL, hid, Tile32::unpack(p), 100);
                    if let Some(n) = nu {
                        self.um(n).o.flags.by_scenario = true;
                        let d = self.map_find_location_tile(location_id, hid);
                        let e = self.tile_encode(d);
                        self.unit_set_destination(n, e);
                    }
                }
                if let Some(n) = nu {
                    let l = self.u(n).o.linked_id;
                    self.um(u).o.linked_id = l;
                    self.um(n).o.linked_id = u as u8;
                    self.um(n).o.flags.in_transport = true;
                    self.scenario.reinforcement[i].unit_id = INDEX_INVALID;
                    deployed = true;
                } else {
                    self.scenario.reinforcement[i].time_left = 1;
                }
            } else {
                let p = self.map_find_location_tile(location_id, hid);
                deployed = self.unit_set_position(u, Tile32::unpack(p));
            }
            if deployed && r.repeat != 0 {
                self.validate_strict_if_zero += 1;
                let t = self.u(u).o.type_;
                let nu2 = self.unit_create(INDEX_INVALID, t, hid, Tile32::new(0xFFFF, 0xFFFF), 0);
                self.validate_strict_if_zero -= 1;
                if let Some(n) = nu2 {
                    self.scenario.reinforcement[i].unit_id = n;
                    self.scenario.reinforcement[i].time_left = r.time_between;
                }
            }
        }
    }

    /// House_EnsureHarvesterAvailable
    fn house_ensure_harvester_available(&mut self, house_id: u8) {
        let mut find = PoolFind::new(house_id, 0xFFFF);
        while let Some(s) = self.structure_find(&mut find) {
            let t = self.s(s).o.type_;
            if !self.config.dune2_enhanced && t == STRUCTURE_HEAVY_VEHICLE {
                continue;
            }
            if self.config.dune2_enhanced && t == STRUCTURE_CONSTRUCTION_YARD {
                continue;
            }
            if self.s(s).o.linked_id == UNIT_INVALID {
                continue;
            }
            if self.u(self.s(s).o.linked_id as u16).o.type_ == UNIT_HARVESTER {
                return;
            }
        }
        let mut find = PoolFind::new(house_id, UNIT_CARRYALL as u16);
        while let Some(u) = self.unit_find(&mut find) {
            let l = self.u(u).o.linked_id;
            if l == UNIT_INVALID {
                continue;
            }
            if self.u(l as u16).o.type_ == UNIT_HARVESTER {
                return;
            }
        }
        if self.unit_is_type_on_map(house_id, UNIT_HARVESTER) {
            return;
        }
        let mut find = PoolFind::new(house_id, STRUCTURE_REFINERY as u16);
        let Some(s) = self.structure_find(&mut find) else { return };
        let e = self.structure_encode(s);
        if self.unit_create_wrapper(house_id, UNIT_HARVESTER, e).is_none() {
            return;
        }
        if house_id != self.player_house_id {
            return;
        }
        self.display_string(STR_HARVESTER_IS_HEADING_TO_REFINERY, 0);
    }

    /// House_UpdateRadarState
    pub fn house_update_radar_state(&mut self, h: u8) -> bool {
        if h != self.player_house_id {
            return false;
        }
        let house = self.h(h);
        let mut activate = house.flags.radar_activated;
        let has_outpost = house.structures_built & (1 << STRUCTURE_OUTPOST) != 0;
        if house.flags.radar_activated {
            if !has_outpost || house.power_production < house.power_usage {
                activate = false;
            }
        } else if has_outpost && house.power_production >= house.power_usage {
            activate = true;
        }
        if self.h(h).flags.radar_activated == activate {
            return false;
        }
        self.emit(Event::VoiceAtTile(62, Tile32::default()));
        self.feedback(if activate { 28 } else { 29 });
        self.emit(Event::Radar(activate));
        self.hm(h).flags.radar_activated = activate;
        activate
    }

    /// House_UpdateCreditsStorage
    pub fn house_update_credits_storage(&mut self, house_id: u8) {
        let old = self.validate_strict_if_zero;
        self.validate_strict_if_zero = 0;
        let mut storage: u32 = 0;
        let mut find = PoolFind::new(house_id, 0xFFFF);
        while let Some(s) = self.structure_find(&mut find) {
            storage += self.si(s).credits_storage as u32;
        }
        self.hm(house_id).credits_storage = storage.min(32000) as u16;
        self.validate_strict_if_zero = old;
    }

    /// House_CalculatePowerAndCredit
    pub fn house_calculate_power_and_credit(&mut self, h: u8) {
        {
            let hh = self.hm(h);
            hh.power_usage = 0;
            hh.power_production = 0;
            hh.credits_storage = 0;
        }
        let mut find = PoolFind::new(h, 0xFFFF);
        while let Some(s) = self.structure_find(&mut find) {
            if self.config.dune2_enhanced && self.s(s).o.flags.is_not_on_map {
                continue;
            }
            let si = *self.si(s);
            let hp = self.s(s).o.hitpoints;
            let enhanced = self.config.dune2_enhanced;
            let hh = self.hm(h);
            hh.credits_storage = hh.credits_storage.wrapping_add(si.credits_storage);
            if si.power_usage >= 0 {
                hh.power_usage = hh.power_usage.wrapping_add(si.power_usage as u16);
                continue;
            }
            let prod = (-si.power_usage) as u16;
            if hp >= si.o.hitpoints {
                hh.power_production = hh.power_production.wrapping_add(prod);
                continue;
            }
            if !enhanced && hp <= si.o.hitpoints / 2 {
                hh.power_production = hh.power_production.wrapping_add(prod / 2);
                continue;
            }
            hh.power_production = hh.power_production.wrapping_add((prod as u32 * hp as u32 / si.o.hitpoints as u32) as u16);
        }
        if h == self.player_house_id && self.h(h).power_usage > self.h(h).power_production {
            self.display_string(STR_INSUFFICIENT_POWER_WINDTRAP_IS_NEEDED, 1);
        }
        // Skirmish: an undeployed MCV counts as a base (the campaign always started with one).
        let has_mcv = self.skirmish.enabled && self.unit_find.iter().any(|&u| self.u(u).o.house_id == h && self.u(u).o.type_ == UNIT_MCV);
        if h == self.player_house_id && self.h(h).structures_built == 0 && self.validate_strict_if_zero == 0 && !has_mcv {
            self.player_credits_no_silo = 0;
        }
    }
}
