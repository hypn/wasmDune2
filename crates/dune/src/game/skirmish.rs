//! Skirmish mode: NOT part of the original game.
//!
//! The original only had scripted campaign scenarios. This module sets up a "play against the
//! computer" game (random map, MCV starts) and adds the base-building decisions the original
//! AI never had. Everything else (unit behaviour, attack scripts, production, repair, ...) is
//! the original AI. All hooks are no-ops unless `skirmish.enabled` is set.

use super::map::*;
use super::*;
use crate::data::Assets;
use crate::map::landscape::create_landscape;
use crate::tables::landscape::{Lst, LANDSCAPE_INFO};
use crate::tables::{STRUCTURE_LAYOUT_TILES, STRUCTURE_LAYOUT_TILES_AROUND, STRUCTURE_LAYOUT_TILE_COUNT};
use crate::tile::{pack_xy, packed_x, packed_y};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartForce {
    /// Only an MCV.
    McvOnly,
    /// MCV plus a few light units.
    Small,
    /// MCV plus a medium force.
    Medium,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiDifficulty {
    Easy,
    Normal,
    Hard,
}

#[derive(Clone, Debug)]
pub struct SkirmishSettings {
    pub player_house: u8,
    pub opponents: Vec<u8>,
    pub map_seed: u32,
    /// 0 = 62x62 (original full map), 1 = 32x32.
    pub map_scale: u16,
    pub credits: u16,
    /// 1..=9, equivalent to the campaign mission number: limits structures and upgrades.
    pub tech_level: u16,
    pub difficulty: AiDifficulty,
    pub start_force: StartForce,
    /// Original rule: every computer house is allied against the player.
    /// When false, it's a free-for-all.
    pub ai_allied: bool,
    pub game_speed: u16,
    pub dune2_enhanced: bool,
}

impl Default for SkirmishSettings {
    fn default() -> Self {
        SkirmishSettings {
            player_house: HOUSE_ATREIDES,
            opponents: vec![HOUSE_HARKONNEN],
            map_seed: 1,
            map_scale: 0,
            credits: 1500,
            tech_level: 9,
            difficulty: AiDifficulty::Normal,
            start_force: StartForce::Small,
            ai_allied: true,
            game_speed: 4,
            dune2_enhanced: true,
        }
    }
}

/// Runtime skirmish state, kept separate from the original globals.
#[derive(Clone, Debug, Default)]
pub struct SkirmishState {
    pub enabled: bool,
    pub ai_allied: bool,
    /// Campaign number used for the AI build-speed cap (original: g_campaignID * 20 + 95).
    pub ai_speed_campaign: u16,
    /// Like g_playerCreditsNoSilo, for AI houses (their starting credits survive until they have storage).
    pub ai_credits_no_silo: [u16; HOUSE_INDEX_MAX],
    pub start_positions: Vec<(u8, u16)>,
    pub finished: Option<bool>,
    pub next_ai_tick: u32,
    /// Per computer house: the most enemy ornithopters it has seen over its base at once (capped at 4).
    /// It never goes down: a human who has been raided from the air keeps the turrets (see
    /// `skirmish_ai_pick_structure`).
    pub air_threat: [u8; HOUSE_INDEX_MAX],
    /// Per computer house: the structure an enemy ornithopter was last nearest to, 0 = none.
    pub air_hot: [u16; HOUSE_INDEX_MAX],
    /// Per computer house: where it plans to put the structure it is building (type, position), after asking the
    /// idle units standing there to move (see `skirmish_plan_placement`).
    pub planned: [Option<(u8, u16)>; HOUSE_INDEX_MAX],
}

/// What a computer house wants in its base, in order: (structure, how many).
const BUILD_ORDER: &[(u8, u16)] = &[
    (STRUCTURE_WINDTRAP, 1),
    (STRUCTURE_REFINERY, 1),
    (STRUCTURE_WINDTRAP, 2),
    (STRUCTURE_LIGHT_VEHICLE, 1),
    (STRUCTURE_BARRACKS, 1),
    (STRUCTURE_OUTPOST, 1),
    (STRUCTURE_WOR_TROOPER, 1),
    (STRUCTURE_SILO, 1),
    (STRUCTURE_HEAVY_VEHICLE, 1),
    (STRUCTURE_REFINERY, 2),
    (STRUCTURE_WINDTRAP, 3),
    (STRUCTURE_TURRET, 2),
    (STRUCTURE_REPAIR, 1),
    (STRUCTURE_HIGH_TECH, 1),
    (STRUCTURE_ROCKET_TURRET, 2),
    (STRUCTURE_WINDTRAP, 4),
    (STRUCTURE_HOUSE_OF_IX, 1),
    (STRUCTURE_PALACE, 1),
    (STRUCTURE_ROCKET_TURRET, 4),
    (STRUCTURE_WINDTRAP, 5),
    (STRUCTURE_SILO, 2),
    (STRUCTURE_REFINERY, 3),
    (STRUCTURE_HEAVY_VEHICLE, 2),
    (STRUCTURE_ROCKET_TURRET, 6),
];

/// The starport's stock at the start ([CHOAM] in the scenario INI, g_starportAvailable): the
/// original's last scenario of each tech level (they are the same for every house). Scenario 14
/// is the first with a starport; lower tech levels can't build one.
fn skirmish_starport_stock(tech_level: u16) -> &'static [(u8, i16)] {
    match tech_level {
        0..=6 => &[(UNIT_TRIKE, 5), (UNIT_QUAD, 5), (UNIT_TANK, 4), (UNIT_LAUNCHER, 3), (UNIT_HARVESTER, 2), (UNIT_MCV, 2), (UNIT_CARRYALL, 2)],
        7 => &[(UNIT_TRIKE, 5), (UNIT_QUAD, 5), (UNIT_TANK, 5), (UNIT_SIEGE_TANK, 3), (UNIT_LAUNCHER, 4), (UNIT_HARVESTER, 2), (UNIT_MCV, 2), (UNIT_CARRYALL, 2)],
        8 => &[(UNIT_TRIKE, 5), (UNIT_QUAD, 5), (UNIT_TANK, 5), (UNIT_SIEGE_TANK, 4), (UNIT_LAUNCHER, 4), (UNIT_HARVESTER, 2), (UNIT_MCV, 2), (UNIT_CARRYALL, 2), (UNIT_ORNITHOPTER, 3)],
        _ => &[(UNIT_TRIKE, 5), (UNIT_QUAD, 5), (UNIT_TANK, 6), (UNIT_LAUNCHER, 5), (UNIT_SIEGE_TANK, 6), (UNIT_HARVESTER, 4), (UNIT_MCV, 2), (UNIT_ORNITHOPTER, 5), (UNIT_CARRYALL, 2)],
    }
}

/// A small xorshift generator seeded from the map seed, so map features (blooms, worms) depend on
/// the seed alone. `next(max)` returns 0..=max.
fn seeded_rng(map_seed: u32, salt: u32) -> impl FnMut(u16) -> u16 {
    let mut state = (map_seed ^ salt).wrapping_mul(2654435761) | 1;
    move |max| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        ((state >> 8) % (max as u32 + 1)) as u16
    }
}

impl Game {
    /// Set up a new skirmish game (replaces Game_LoadScenario + Scenario_Load).
    pub fn start_skirmish(&mut self, assets: &Assets, settings: &SkirmishSettings) {
        self.game_init();
        self.config.game_speed = settings.game_speed;
        self.config.dune2_enhanced = settings.dune2_enhanced;
        self.validate_strict_if_zero += 1;

        self.campaign_id = settings.tech_level.clamp(1, 9) - 1;
        self.scenario_id = 22;
        self.scenario = Scenario { win_flags: 3, lose_flags: 1, map_seed: settings.map_seed, map_scale: settings.map_scale.min(2), ..Default::default() };
        self.skirmish = SkirmishState {
            enabled: true,
            ai_allied: settings.ai_allied,
            ai_speed_campaign: match settings.difficulty {
                AiDifficulty::Easy => 2,
                AiDifficulty::Normal => 5,
                AiDifficulty::Hard => 8,
            },
            ..Default::default()
        };

        // Map: many seeds are almost all sand (the campaign hand-picked its seeds), so skip
        // forward to the first seed with a rocky plateau for every house.
        let houses_count = 1 + settings.opponents.len();
        let mut seed = settings.map_seed;
        for _ in 0..500 {
            let mut map = crate::map::Map::default();
            create_landscape(&mut map, &mut self.rng, &assets.tiles, seed);
            self.map = map.tiles;
            self.map_tile_id = map.map_tile_id;
            if self.skirmish_map_viable(houses_count) {
                break;
            }
            seed = (seed + 1) & 0x7FFF;
        }
        self.scenario.map_seed = seed;
        for r in self.scenario.reinforcement.iter_mut() {
            r.unit_id = INDEX_INVALID;
        }
        for &(t, n) in skirmish_starport_stock(settings.tech_level) {
            self.starport_available[t as usize] = n;
        }

        // Houses
        let mut houses = vec![settings.player_house];
        for &h in &settings.opponents {
            if !houses.contains(&h) && h < HOUSE_MAX {
                houses.push(h);
            }
        }
        let ai_count = (houses.len() - 1).max(1) as u16;
        let ai_max_units = (60 / ai_count).clamp(15, 30);
        for (i, &h) in houses.iter().enumerate() {
            self.house_allocate(h);
            let house = self.hm(h);
            house.credits = settings.credits;
            if i == 0 {
                house.flags.human = true;
                house.unit_count_max = 40;
            } else {
                house.unit_count_max = ai_max_units;
                house.flags.is_ai_active = true;
            }
        }
        self.player_house_id = settings.player_house;
        self.player_credits_no_silo = settings.credits;
        for &h in &houses[1..] {
            self.skirmish.ai_credits_no_silo[h as usize] = settings.credits;
        }

        // Starting positions and forces
        let starts = self.skirmish_choose_starts(houses.len());
        for (i, &h) in houses.iter().enumerate() {
            let p = starts[i % starts.len()];
            self.skirmish.start_positions.push((h, p));
            self.skirmish_spawn_force(h, p, settings.start_force);
        }

        self.skirmish_place_blooms(&starts);
        self.skirmish_place_worms(&starts);

        self.tick_scenario_start = self.timer_game;
        self.validate_strict_if_zero -= 1;
        self.game_prepare();

        let p = starts[0];
        self.map_set_viewport_position(p);
        self.minimap_position = self.viewport_position;
        self.selection_position = p;
        self.selection_type = SelectionType::Structure;
    }

    /// Scatter 0-2 spice blooms (the scenario's `Bloom=` list in the original) on plain sand,
    /// away from every start and from each other (Scenario_Load_Map_Bloom). They come from the
    /// map seed alone (not the game RNG), so a seed always has the same blooms.
    fn skirmish_place_blooms(&mut self, starts: &[u16]) {
        let (min_x, min_y, sx, sy) = self.map_info();
        let mut next = seeded_rng(self.scenario.map_seed as u32, 0xB100_0D5E);
        let count = next(2);
        let mut placed: Vec<u16> = Vec::new();
        for _ in 0..count {
            for _ in 0..200 {
                let p = pack_xy(min_x + 2 + next(sx - 5), min_y + 2 + next(sy - 5));
                if !self.map_is_valid_position(p) || self.map_get_landscape_type(p) != Lst::NormalSand as u16 {
                    continue;
                }
                if starts.iter().chain(placed.iter()).any(|&o| get_distance_packed(p, o) < 12) {
                    continue;
                }
                self.map[p as usize].ground_tile_id = self.bloom_tile_id;
                self.map_tile_id[p as usize] |= 0x8000;
                placed.push(p);
                break;
            }
        }
    }

    /// Place 1-2 Fremen sandworms on sand (the scenario's `Fremen,Sandworm,256,<tile>,64,Area Guard`
    /// units), well away from every start and from each other. Seed-derived like the blooms.
    fn skirmish_place_worms(&mut self, starts: &[u16]) {
        let (min_x, min_y, sx, sy) = self.map_info();
        if self.house_allocate(HOUSE_FREMEN).is_some() {
            self.hm(HOUSE_FREMEN).unit_count_max = 10;
        }
        let mut next = seeded_rng(self.scenario.map_seed as u32, 0x5A4D_0F0B);
        let count = 1 + next(1);
        let mut placed: Vec<u16> = Vec::new();
        for _ in 0..count {
            for _ in 0..300 {
                let p = pack_xy(min_x + 2 + next(sx - 5), min_y + 2 + next(sy - 5));
                if !self.map_is_valid_position(p) || !self.landscape_info(p).is_sand {
                    continue;
                }
                if starts.iter().any(|&o| get_distance_packed(p, o) < 16) || placed.iter().any(|&o| get_distance_packed(p, o) < 20) {
                    continue;
                }
                let Some(u) = self.unit_create(INDEX_INVALID, UNIT_SANDWORM, HOUSE_FREMEN, Tile32::unpack(p), 64) else { continue };
                self.um(u).o.flags.by_scenario = true;
                self.um(u).o.seen_by_houses = 0;
                // Scenario units skip Unit_Create (which zeroes it), keeping Unit_Allocate's three meals.
                self.um(u).amount = 3;
                self.unit_set_action(u, ACTION_AREA_GUARD);
                placed.push(p);
                break;
            }
        }
    }

    /// Skirmish addition: note when enemy ornithopters are over a computer base (within 16 tiles of one of
    /// its structures), for `skirmish_ai_pick_structure` and the placement of turrets.
    fn skirmish_track_air_threat(&mut self) {
        let orni: Vec<u16> = self.unit_find.iter().copied().filter(|&u| self.u(u).o.type_ == UNIT_ORNITHOPTER && !self.u(u).o.flags.is_not_on_map).collect();
        if orni.is_empty() {
            return;
        }
        for h in self.house_ids() {
            if self.h(h).flags.human || h == HOUSE_FREMEN {
                continue;
            }
            let structures: Vec<u16> = self.structure_find.iter().copied().filter(|&s| {
                let st = self.s(s);
                st.o.house_id == h && !st.o.flags.is_not_on_map && !matches!(st.o.type_, STRUCTURE_SLAB_1X1 | STRUCTURE_SLAB_2X2 | STRUCTURE_WALL)
            }).collect();
            let mut count = 0u8;
            let mut hot: Option<(u16, u16)> = None;
            for &u in &orni {
                if self.house_are_allied(h, self.u(u).o.house_id) {
                    continue;
                }
                let at = self.u(u).o.position.pack();
                let nearest = structures.iter().map(|&s| (get_distance_packed(at, self.s(s).o.position.pack()), self.s(s).o.position.pack())).min();
                if let Some((d, pos)) = nearest {
                    if d <= 16 {
                        count += 1;
                        if hot.map_or(true, |(hd, _)| d < hd) {
                            hot = Some((d, pos));
                        }
                    }
                }
            }
            if let Some((_, pos)) = hot {
                self.skirmish.air_hot[h as usize] = pos;
                self.skirmish.air_threat[h as usize] = self.skirmish.air_threat[h as usize].max(count.min(4));
            }
        }
    }

    /// Skirmish addition: an enemy structure counts as seen by the player (so ornithopters, and units on guard
    /// or hunting, will target it) once any of its tiles is uncovered. In the original that only happens at
    /// the moment a tile is first uncovered (Map_UnveilTile), so a structure the computer builds on ground
    /// the player has already seen never counts: units are re-marked whenever they move, structures never.
    fn skirmish_see_structures(&mut self) {
        let bit = 1u8 << self.player_house_id;
        for s in self.structure_find.clone() {
            let st = self.s(s);
            if st.o.seen_by_houses & bit != 0 || st.o.flags.is_not_on_map {
                continue;
            }
            let layout = self.si(s).layout as usize;
            let position = st.o.position.pack();
            let seen = (0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize).any(|j| {
                let p = position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]) & 0xFFF;
                let t = &self.map[p as usize];
                t.is_unveiled && self.tile_is_unveiled(t.overlay_tile_id)
            });
            if seen {
                self.sm(s).o.seen_by_houses |= bit;
            }
        }
    }

    /// Skirmish addition: the original pathfinder treats every tile with another unit on it as blocked and
    /// nothing ever asks that unit to move, so an idle unit in the only way out of a base keeps a unit that
    /// wants to leave (a quad sent to attack, say) shut in for good. A computer ground unit that has
    /// somewhere to go and has not moved for six seconds asks the idle friendly units next to it to step
    /// aside, to the free tile near them that is furthest from it.
    fn skirmish_unblock_units(&mut self) {
        const DIRS: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];
        for u in self.unit_find.clone() {
            let un = self.u(u);
            let h = un.o.house_id;
            if un.o.flags.is_not_on_map || h == self.player_house_id || h == HOUSE_FREMEN || self.h(h).flags.human {
                continue;
            }
            let ui = self.ui(u);
            if !ui.flags.is_ground_unit || ui.movement_type == MOVEMENT_WINGER || un.o.type_ == UNIT_SANDWORM {
                continue;
            }
            // An aggressive unit with something to attack that is not moving and not shooting. (A unit the
            // pathfinder cannot route forgets its destination, so look at the action, not the destination.)
            let aggressive = matches!(un.action_id, ACTION_HUNT | ACTION_ATTACK);
            let moving = un.speed != 0 || un.current_destination.x != 0 || un.current_destination.y != 0;
            let firing = un.target_attack != 0 && self.object_get_distance_to_encoded(ObjRef::Unit(u), un.target_attack) <= ui.fire_distance << 8;
            if !aggressive || moving || firing {
                self.um(u).stuck_secs = 0;
                continue;
            }
            if self.u(u).target_move == 0 && self.u(u).target_attack == 0 && self.unit_find_best_target_encoded(u, 0) == 0 {
                self.um(u).stuck_secs = 0;
                continue;
            }
            self.um(u).stuck_secs = self.u(u).stuck_secs.saturating_add(1);
            if self.u(u).stuck_secs < 6 {
                continue;
            }
            self.um(u).stuck_secs = 3;
            let here = self.u(u).o.position.pack();
            let (x, y) = (packed_x(here) as i32, packed_y(here) as i32);
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if !(0..64).contains(&nx) || !(0..64).contains(&ny) {
                    continue;
                }
                let Some(b) = self.unit_get_by_packed_tile(pack_xy(nx as u16, ny as u16)) else { continue };
                if b == u {
                    continue;
                }
                let bn = self.u(b);
                let bi = self.ui(b);
                let idle = bn.speed == 0
                    && bn.target_move == 0
                    && bn.target_attack == 0
                    && bn.current_destination.x == 0
                    && bn.current_destination.y == 0
                    && matches!(bn.action_id, ACTION_GUARD | ACTION_AREA_GUARD | ACTION_STOP | ACTION_AMBUSH);
                if !idle || !bi.flags.is_ground_unit || bn.o.flags.is_not_on_map || !self.house_are_allied(bn.o.house_id, h) || bn.o.type_ == UNIT_SANDWORM {
                    continue;
                }
                // The free tile within three of the blocker, furthest from the unit it is blocking.
                let (bx, by) = (packed_x(self.u(b).o.position.pack()) as i32, packed_y(self.u(b).o.position.pack()) as i32);
                let mt = bi.movement_type as usize;
                let mut best: Option<(u16, u16)> = None;
                for ty in -3i32..=3 {
                    for tx in -3i32..=3 {
                        let (px, py) = (bx + tx, by + ty);
                        if !(0..64).contains(&px) || !(0..64).contains(&py) {
                            continue;
                        }
                        let p = pack_xy(px as u16, py as u16);
                        if !self.map_is_valid_position(p) || self.object_get_by_packed_tile(p).is_some() || self.landscape_info(self.map_get_landscape_type(p)).movement_speed[mt] == 0 {
                            continue;
                        }
                        let d = get_distance_packed(p, here);
                        if best.map_or(true, |(_, bd)| d > bd) {
                            best = Some((p, d));
                        }
                    }
                }
                if let Some((p, d)) = best {
                    if d >= 2 {
                        self.unit_set_action(b, ACTION_MOVE);
                        let e = self.tile_encode(p);
                        self.unit_set_destination(b, e);
                        self.um(b).origin_encoded = 0;
                    }
                }
            }
        }
    }

    /// Skirmish addition: the harvester script stops (for good) a harvester that finds no spice, and a stopped
    /// one never looks again. Every ten seconds, send the computers' stopped harvesters back to work (or home
    /// with what they carry); the script stops them again if there is still no spice anywhere.
    fn skirmish_restart_harvesters(&mut self) {
        if self.timer_game % 600 >= 60 {
            return;
        }
        for u in self.unit_find.clone() {
            let un = self.u(u);
            if un.o.type_ != UNIT_HARVESTER || un.o.flags.is_not_on_map || un.action_id != ACTION_STOP || un.o.house_id == self.player_house_id {
                continue;
            }
            let action = if un.amount >= 100 { ACTION_RETURN } else { ACTION_HARVEST };
            self.unit_set_action(u, action);
        }
    }

    /// Skirmish addition: the original worm only ever chases a target, and otherwise sits still.
    /// An idle worm picks a random sand tile near where it started (inside the script's own
    /// 10 tile leash around its origin) and heads there, as if it were a target.
    fn skirmish_worms_wander(&mut self) {
        for u in self.units_of(HOUSE_FREMEN, UNIT_SANDWORM as u16) {
            let un = self.u(u);
            if !un.o.flags.used || un.action_id != ACTION_AREA_GUARD {
                continue;
            }
            // Walking to a wander tile: drop it as soon as there is something to eat (the script
            // only looks for a better target while its target is an enemy).
            if un.target_attack != 0 && Self::index_get_type(un.target_attack) == IT_TILE {
                if self.unit_sandworm_find_best_target(u).is_some() {
                    let un = self.um(u);
                    un.target_attack = 0;
                    un.target_move = 0;
                    un.route[0] = 0xFF;
                }
                continue;
            }
            if un.target_attack != 0 || un.target_move != 0 {
                continue;
            }
            if self.unit_sandworm_find_best_target(u).is_some() || self.rng.random_256() & 3 != 0 {
                continue;
            }
            let origin = if self.u(u).origin_encoded != 0 { self.index_get_tile(self.u(u).origin_encoded) } else { self.u(u).o.position };
            let o = origin.pack();
            let (ox, oy) = (packed_x(o) as i32, packed_y(o) as i32);
            for _ in 0..8 {
                let x = ox + self.rng.lcg_range(0, 16) as i32 - 8;
                let y = oy + self.rng.lcg_range(0, 16) as i32 - 8;
                if !(0..64).contains(&x) || !(0..64).contains(&y) {
                    continue;
                }
                let p = pack_xy(x as u16, y as u16);
                if !self.map_is_valid_position(p) || !self.landscape_info(p).is_sand || get_distance_packed(p, o) > 8 {
                    continue;
                }
                let e = self.tile_encode(p);
                self.unit_set_target(u, e);
                break;
            }
        }
    }

    /// Does the current map have `count` well separated plateaus to start on?
    fn skirmish_map_viable(&mut self, count: usize) -> bool {
        let saved = self.rng.clone();
        let cands = self.skirmish_start_candidates();
        self.rng = saved;
        let (_, _, sx, _) = self.map_info();
        let min_d = (sx / 3).max(10);
        let mut chosen: Vec<u16> = Vec::new();
        for (p, _) in cands {
            if chosen.iter().all(|&c| get_distance_packed(p, c) >= min_d) {
                chosen.push(p);
                if chosen.len() >= count {
                    return true;
                }
            }
        }
        false
    }

    /// Spread starting positions over rocky plateaus, as far apart as possible.
    fn skirmish_choose_starts(&mut self, count: usize) -> Vec<u16> {
        let mut candidates = self.skirmish_start_candidates();
        let (min_x, min_y, sx, sy) = self.map_info();
        if candidates.is_empty() {
            // No rock at all: fall back to evenly spaced positions.
            for y in (min_y + 3..min_y + sy - 3).step_by(4) {
                for x in (min_x + 3..min_x + sx - 3).step_by(4) {
                    candidates.push((pack_xy(x, y), 1));
                }
            }
        }
        let mut chosen: Vec<u16> = Vec::new();
        let first = candidates[self.rng.lcg_range(0, candidates.len() as u16 - 1) as usize].0;
        chosen.push(first);
        while chosen.len() < count {
            let best = candidates
                .iter()
                .max_by_key(|(p, score)| {
                    let d = chosen.iter().map(|&c| get_distance_packed(*p, c) as u32).min().unwrap_or(0);
                    d * 64 + (*score).min(63)
                })
                .map(|c| c.0)
                .unwrap_or(first);
            chosen.push(best);
        }
        chosen
    }

    /// Candidate CY positions (top-left) whose 4x4 area is rock, scored by nearby rock.
    fn skirmish_start_candidates(&self) -> Vec<(u16, u32)> {
        let (min_x, min_y, sx, sy) = self.map_info();
        let is_rock = |g: &Game, x: u16, y: u16| -> bool {
            let p = pack_xy(x, y);
            g.map_is_valid_position(p) && LANDSCAPE_INFO[g.map_get_landscape_type(p) as usize].is_valid_for_structure
        };
        // Candidate = top-left of the CY whose 4x4 neighbourhood is rock; score = rock tiles nearby.
        let mut candidates: Vec<(u16, u32)> = Vec::new();
        for y in (min_y + 2)..(min_y + sy).saturating_sub(4) {
            for x in (min_x + 2)..(min_x + sx).saturating_sub(4) {
                if !(0..4).all(|j| (0..4).all(|i| is_rock(self, x + i, y + j))) {
                    continue;
                }
                let mut score = 0u32;
                for j in 0..9u16 {
                    for i in 0..9u16 {
                        let (xx, yy) = ((x + i).wrapping_sub(3), (y + j).wrapping_sub(3));
                        if xx < 64 && yy < 64 && is_rock(self, xx, yy) {
                            score += 1;
                        }
                    }
                }
                if score >= 40 {
                    candidates.push((pack_xy(x, y), score));
                }
            }
        }
        candidates
    }

    fn skirmish_spawn_force(&mut self, h: u8, packed: u16, force: StartForce) {
        // MCV in the bottom-right tile of the 2x2 CY footprint (deploy tries offsets 0,-1,-64,-65).
        let mcv_pos = packed + 65;
        let o = self.rng.random_256() as i8;
        if let Some(u) = self.unit_create(INDEX_INVALID, UNIT_MCV, h, Tile32::unpack(mcv_pos), o) {
            if h != self.player_house_id {
                self.unit_set_action(u, ACTION_DEPLOY);
            }
        }
        let extra: &[u8] = match force {
            StartForce::McvOnly => &[],
            StartForce::Small => &[UNIT_TRIKE, UNIT_INFANTRY, UNIT_INFANTRY],
            StartForce::Medium => &[UNIT_QUAD, UNIT_TRIKE, UNIT_TANK, UNIT_INFANTRY, UNIT_INFANTRY, UNIT_TROOPERS],
        };
        let mut ring = 0u16;
        for &t in extra {
            let mut t = t;
            if t == UNIT_TRIKE && h == HOUSE_ORDOS {
                t = UNIT_RAIDER_TRIKE;
            }
            if self.unit_info[t as usize].o.available_house & (1 << h) == 0 {
                t = if self.unit_info[t as usize].movement_type == MOVEMENT_FOOT { UNIT_SOLDIER } else { UNIT_QUAD };
            }
            // Find a free passable tile around the base location.
            for _ in 0..64 {
                let around = STRUCTURE_LAYOUT_TILES_AROUND[6][(ring % 16) as usize];
                ring += 1;
                if around == 0 {
                    continue;
                }
                let p = (packed as i32 + around as i32 + 65 * ((ring / 16) as i32 % 3)) as u16 & 0xFFF;
                if !self.map_is_valid_position(p) || self.object_get_by_packed_tile(p).is_some() {
                    continue;
                }
                let o = self.rng.random_256() as i8;
                if let Some(u) = self.unit_create(INDEX_INVALID, t, h, Tile32::unpack(p), o) {
                    // Starting units defend the base rather than rushing (original AI default: hunt).
                    if h != self.player_house_id {
                        self.unit_set_action(u, ACTION_AREA_GUARD);
                    }
                    break;
                }
            }
        }
    }

    /// DEBUG: remove the fog of war for the player (unveils every tile, like exploring the map).
    pub fn debug_reveal_map(&mut self) {
        let p = self.player_house_id;
        for i in 0..4096u16 {
            if self.map_is_valid_position(i) {
                self.map_unveil_tile(i, p);
            }
        }
    }

    /// Game_Prepare (the parts that matter for a freshly created game).
    pub fn game_prepare(&mut self) {
        self.validate_strict_if_zero += 1;
        let old = self.selection_type;
        self.selection_type = SelectionType::Mentat;
        self.structure_recount();
        self.unit_recount();
        self.team_recount();
        for i in 0..4096u16 {
            let u = self.unit_get_by_packed_tile(i);
            let s = self.structure_get_by_packed_tile(i);
            if u.map_or(true, |u| !self.u(u).o.flags.used) {
                self.map[i as usize].has_unit = false;
            }
            if s.map_or(true, |s| !self.s(s).o.flags.used) {
                self.map[i as usize].has_structure = false;
            }
            if self.map[i as usize].is_unveiled {
                let p = self.player_house_id;
                self.map_unveil_tile(i, p);
            }
        }
        for u in self.units_of(HOUSE_INVALID, 0xFFFF) {
            if self.u(u).o.flags.is_not_on_map {
                continue;
            }
            self.unit_remove_fog(u);
            self.unit_update_map(1, u);
        }
        for s in self.structures_of(HOUSE_INVALID, 0xFFFF) {
            let t = self.s(s).o.type_;
            if t == STRUCTURE_SLAB_1X1 || t == STRUCTURE_SLAB_2X2 || t == STRUCTURE_WALL || self.s(s).o.flags.is_not_on_map {
                continue;
            }
            self.structure_remove_fog(s);
            self.script_load(super::script::Ctx::Structure(s), t);
            let h = self.s(s).o.house_id;
            if t == STRUCTURE_PALACE || (self.h(h).palace_position.x == 0 && self.h(h).palace_position.y == 0) {
                let p = self.s(s).o.position;
                self.hm(h).palace_position = p;
            }
        }
        for h in self.house_ids() {
            let b = self.structure_get_structures_built(h);
            self.hm(h).structures_built = b;
            self.house_update_credits_storage(h);
            self.house_calculate_power_and_credit(h);
        }
        self.ticks.house_power_maintenance = self.ticks.house_power_maintenance.max(self.timer_game + 70);
        self.player_credits = 0xFFFF;
        self.selection_type = old;
        self.validate_strict_if_zero -= 1;
    }

    // ---------------------------------------------------------------------------------------
    // AI extensions
    // ---------------------------------------------------------------------------------------

    fn skirmish_count_structures(&self, h: u8, t: u8) -> u16 {
        self.structure_find
            .iter()
            .filter(|&&s| {
                let st = self.s(s);
                st.o.house_id == h && st.o.type_ == t
            })
            .count() as u16
    }

    /// Forget the rebuild spots (ai_structureRebuild) something now stands on. The original AI
    /// rebuilds a destroyed structure on its old spot without checking it (Structure_Place skips
    /// the check for computer houses), which was safe because it built nothing else. The skirmish
    /// AI builds new structures near its base, possibly on such a spot, and a rebuild there
    /// would overlap them.
    pub fn skirmish_drop_blocked_rebuilds(&mut self, h: u8, planning: bool) {
        if !self.skirmish.enabled {
            return;
        }
        for i in 0..5 {
            let [t, position] = self.h(h).ai_structure_rebuild[i];
            if t == 0 {
                continue;
            }
            let layout = self.structure_info[t as usize].layout as usize;
            let blocked = (0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize).any(|j| {
                let cur = position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]);
                !self.map_is_valid_position(cur) || self.map[(cur & 0xFFF) as usize].has_structure || self.map_get_landscape_type(cur) == LST_WALL
            }) || self.skirmish_seals_ground(t as u8, position, h, true);
            let units = self.skirmish_footprint_units(t as u8, position);
            let units_in_way = if planning { units.iter().any(|&u| !self.skirmish_unit_movable(u, h)) } else { !units.is_empty() };
            if !blocked && planning && !units.is_empty() && !units_in_way {
                // Only idle units of ours: ask them to move now, before the structure is done.
                self.skirmish_clear_footprint(h, t as u8, position);
            }
            if blocked || units_in_way {
                self.hm(h).ai_structure_rebuild[i] = [0, 0];
            }
        }
    }

    /// Which structure should a computer CY build next (when it has nothing to rebuild)?
    pub fn skirmish_ai_pick_structure(&mut self, h: u8, buildable: u32) -> u16 {
        if !self.skirmish.enabled || self.h(h).flags.human {
            return 0xFFFF;
        }
        let can = |t: u8| buildable & (1 << t) != 0;
        // Low power first.
        let house = self.h(h);
        if can(STRUCTURE_WINDTRAP) && house.power_usage > 0 && house.power_production < house.power_usage + 20 {
            return STRUCTURE_WINDTRAP as u16;
        }
        // Spice storage nearly full: silo.
        if can(STRUCTURE_SILO) && house.credits_storage != 0 && house.credits as u32 * 10 > house.credits_storage as u32 * 8 {
            return STRUCTURE_SILO as u16;
        }
        // Raided from the air: more turrets, as a human would. Two rocket turrets (cannon turrets without
        // them) plus two more for each ornithopter seen over the base at once, up to ten.
        let threat = self.skirmish.air_threat[h as usize] as u16;
        if threat > 0 {
            let t = if can(STRUCTURE_ROCKET_TURRET) { Some(STRUCTURE_ROCKET_TURRET) } else if can(STRUCTURE_TURRET) { Some(STRUCTURE_TURRET) } else { None };
            if let Some(t) = t {
                if self.skirmish_count_structures(h, t) < (2 + 2 * threat).min(10) {
                    let usage = self.structure_info[t as usize].power_usage;
                    let house = self.h(h);
                    if usage > 0 && can(STRUCTURE_WINDTRAP) && (house.power_production as i32) < house.power_usage as i32 + usage as i32 {
                        return STRUCTURE_WINDTRAP as u16;
                    }
                    return t as u16;
                }
            }
        }
        for &(t, n) in BUILD_ORDER {
            if !can(t) {
                continue;
            }
            if self.skirmish_count_structures(h, t) < n {
                // Keep power positive before adding consumers.
                let usage = self.structure_info[t as usize].power_usage;
                let house = self.h(h);
                if usage > 0 && can(STRUCTURE_WINDTRAP) && (house.power_production as i32) < house.power_usage as i32 + usage as i32 {
                    return STRUCTURE_WINDTRAP as u16;
                }
                return t as u16;
            }
        }
        0xFFFF
    }

    /// Find a spot for a computer house's finished structure and place it.
    /// Whether a ground unit stands on (or is moving onto) the footprint of a structure of type
    /// `t` at `position`. Structure_Place would delete one unit per tile and leave any others
    /// inside the new building, so the AI does not build there.
    pub fn skirmish_footprint_has_units(&self, t: u8, position: u16) -> bool {
        !self.skirmish_footprint_units(t, position).is_empty()
    }

    /// Whether units stand on the footprint, ignoring (with `movable_ok`) idle ones that can be asked to move.
    fn skirmish_blocked_by_units(&self, t: u8, position: u16, h: u8, movable_ok: bool) -> bool {
        self.skirmish_footprint_units(t, position).iter().any(|&u| !movable_ok || !self.skirmish_unit_movable(u, h))
    }

    /// The ground units on (or moving onto) the footprint of a structure of type `t` at `position`.
    pub fn skirmish_footprint_units(&self, t: u8, position: u16) -> Vec<u16> {
        if !self.skirmish.enabled {
            return Vec::new();
        }
        let layout = self.structure_info[t as usize].layout as usize;
        let footprint: Vec<u16> =
            (0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize).map(|j| position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]) & 0xFFF).collect();
        self.unit_find
            .iter()
            .copied()
            .filter(|&u| {
                let un = self.u(u);
                if un.o.flags.is_not_on_map || !self.ui(u).flags.is_ground_unit {
                    return false;
                }
                let dest = un.current_destination;
                footprint.contains(&un.o.position.pack()) || (dest.x != 0 || dest.y != 0) && footprint.contains(&dest.pack())
            })
            .collect()
    }

    /// Whether unit `u` is one of house `h`'s (or an ally's) ground units standing about with nothing to do, so
    /// that it can be asked to step aside.
    fn skirmish_unit_movable(&self, u: u16, h: u8) -> bool {
        let un = self.u(u);
        un.o.type_ != UNIT_SANDWORM
            && !un.o.flags.is_not_on_map
            && self.house_are_allied(un.o.house_id, h)
            && un.speed == 0
            && un.target_move == 0
            && un.target_attack == 0
            && un.current_destination.x == 0
            && un.current_destination.y == 0
            && matches!(un.action_id, ACTION_GUARD | ACTION_AREA_GUARD | ACTION_STOP | ACTION_AMBUSH)
    }

    /// Ask the (idle) units on the footprint of a structure of type `t` at `position` to move to the nearest
    /// free ground off it and not next to it.
    fn skirmish_clear_footprint(&mut self, h: u8, t: u8, position: u16) {
        let layout = self.structure_info[t as usize].layout as usize;
        let footprint: Vec<u16> =
            (0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize).map(|j| position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]) & 0xFFF).collect();
        let mut taken: Vec<u16> = Vec::new();
        for u in self.skirmish_footprint_units(t, position) {
            if !self.skirmish_unit_movable(u, h) {
                continue;
            }
            let here = self.u(u).o.position.pack();
            let (hx, hy) = (packed_x(here) as i32, packed_y(here) as i32);
            let mt = self.ui(u).movement_type as usize;
            let mut best: Option<(u16, u16)> = None;
            for dy in -4i32..=4 {
                for dx in -4i32..=4 {
                    let (x, y) = (hx + dx, hy + dy);
                    if !(0..64).contains(&x) || !(0..64).contains(&y) {
                        continue;
                    }
                    let p = pack_xy(x as u16, y as u16);
                    if taken.contains(&p)
                        || !self.map_is_valid_position(p)
                        || self.object_get_by_packed_tile(p).is_some()
                        || self.landscape_info(self.map_get_landscape_type(p)).movement_speed[mt] == 0
                        || footprint.iter().any(|&f| get_distance_packed(f, p) < 2)
                    {
                        continue;
                    }
                    let d = get_distance_packed(here, p);
                    if best.map_or(true, |(_, bd)| d < bd) {
                        best = Some((p, d));
                    }
                }
            }
            if let Some((p, _)) = best {
                taken.push(p);
                self.unit_set_action(u, ACTION_MOVE);
                let e = self.tile_encode(p);
                self.unit_set_destination(u, e);
                self.um(u).origin_encoded = 0;
            }
        }
    }

    /// The centre of a computer house's base: its first construction yard, else its start position.
    fn skirmish_base_center(&self, h: u8) -> u16 {
        let mut find = PoolFind::new(h, STRUCTURE_CONSTRUCTION_YARD as u16);
        match self.structure_find(&mut find) {
            Some(cy) => self.s(cy).o.position.pack() + 65,
            None => self.skirmish.start_positions.iter().find(|(hh, _)| *hh == h).map(|x| x.1).unwrap_or(0),
        }
    }

    /// Called when a computer construction yard starts a structure: if the spot it would like for it has idle
    /// units of its own standing on it, ask them to move now, and remember the spot for when the structure is
    /// done (`skirmish_ai_place_structure`). Without this the AI just built somewhere else, or nowhere.
    pub fn skirmish_plan_placement(&mut self, h: u8, t: u16) {
        self.skirmish.planned[h as usize] = None;
        if !self.skirmish.enabled || t >= STRUCTURE_MAX as u16 || matches!(t as u8, STRUCTURE_SLAB_1X1 | STRUCTURE_SLAB_2X2 | STRUCTURE_WALL) {
            return;
        }
        let t = t as u8;
        let center = self.skirmish_base_center(h);
        let best = self.skirmish_best_spot(t, center, h, 2, true).or_else(|| self.skirmish_best_spot(t, center, h, 1, true));
        let Some((p, _)) = best else { return };
        if self.skirmish_footprint_units(t, p).is_empty() {
            return;
        }
        self.skirmish_clear_footprint(h, t, p);
        self.skirmish.planned[h as usize] = Some((t, p));
    }

    pub fn skirmish_ai_place_structure(&mut self, h: u8, s: u16) -> bool {
        if !self.skirmish.enabled {
            return false;
        }
        let t = self.s(s).o.type_;
        let center = self.skirmish_base_center(h);
        // The spot planned when the structure was started, if its units have moved off it.
        if let Some((pt, pp)) = self.skirmish.planned[h as usize].take() {
            if pt == t
                && self.structure_is_valid_build_location_for(pp, t, h) != 0
                && !self.skirmish_footprint_has_units(t, pp)
                && !self.skirmish_seals_ground(t, pp, h, true)
            {
                self.validate_strict_if_zero += 1;
                let ok = self.structure_place(s, pp);
                self.validate_strict_if_zero -= 1;
                if ok {
                    return true;
                }
            }
        }
        // Never where it would shut a unit into a pocket of ground it could not leave (they would be stuck
        // for good), and preferably not where it would seal off any ground. With no such spot the
        // structure waits for room.
        let best = self.skirmish_best_spot(t, center, h, 2, false).or_else(|| self.skirmish_best_spot(t, center, h, 1, false));
        let Some((p, _)) = best else { return false };
        self.validate_strict_if_zero += 1;
        let ok = self.structure_place(s, p);
        self.validate_strict_if_zero -= 1;
        ok
    }

    /// The best place for a computer house's new structure of type `t` around `center` (see
    /// `skirmish_ai_place_structure`); `seal`: 2 refuses spots that would seal off any ground, 1 only
    /// those that would seal in a ground unit.
    fn skirmish_best_spot(&mut self, t: u8, center: u16, h: u8, seal: u8, movable_ok: bool) -> Option<(u16, i32)> {
        let layout = self.structure_info[t as usize].layout as usize;
        let count = STRUCTURE_LAYOUT_TILE_COUNT[layout] as u16;
        let (cx, cy) = (packed_x(center) as i32, packed_y(center) as i32);
        let is_defense = t == STRUCTURE_TURRET || t == STRUCTURE_ROCKET_TURRET;
        let mut best: Option<(u16, i32)> = None;
        for r in 1..14i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (cx + dx, cy + dy);
                    if !(0..64).contains(&x) || !(0..64).contains(&y) {
                        continue;
                    }
                    let p = pack_xy(x as u16, y as u16);
                    if self.structure_is_valid_build_location_for(p, t, h) == 0 || self.skirmish_blocked_by_units(t, p, h, movable_ok) {
                        continue;
                    }
                    if seal != 0 && self.skirmish_seals_ground(t, p, h, seal == 1) {
                        continue;
                    }
                    // Prefer spots that keep the tiles around the footprint free (paths for units).
                    let mut blocked = 0;
                    for i in 0..16 {
                        let off = STRUCTURE_LAYOUT_TILES_AROUND[layout][i];
                        if off == 0 {
                            break;
                        }
                        let q = p.wrapping_add(off as u16);
                        let lst = self.map_get_landscape_type(q);
                        if lst == LST_STRUCTURE || !LANDSCAPE_INFO[lst as usize].is_valid_for_structure2 && lst != LST_CONCRETE_SLAB {
                            blocked += 1;
                        }
                    }
                    let dist = r * 4;
                    // Turrets go where ornithopters were last over the base, else out on its edge.
                    let hot = self.skirmish.air_hot[h as usize];
                    let score = if is_defense && hot != 0 {
                        get_distance_packed(p, hot) as i32 * 4 + blocked * 3
                    } else if is_defense {
                        -dist + blocked * 3
                    } else {
                        dist + blocked * 6 + count as i32
                    };
                    if best.map_or(true, |(_, b)| score < b) {
                        best = Some((p, score));
                    }
                }
            }
            if !is_defense && best.is_some() && r > 3 {
                break;
            }
        }
        best
    }

    /// Whether a structure of type `t` at `position` would cut a closed pocket (under `POCKET` tiles) off
    /// from the rest of the map that was not closed before: ground that units could never leave. With
    /// `only_with_units`, only a pocket with a ground unit in it counts (closed before or not: the unit
    /// would not get out either way).
    pub fn skirmish_seals_ground(&self, t: u8, position: u16, h: u8, only_with_units: bool) -> bool {
        const POCKET: usize = 40;
        let layout = self.structure_info[t as usize].layout as usize;
        let footprint: Vec<u16> =
            (0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize).map(|j| position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]) & 0xFFF).collect();
        let blocked = |q: u16, with: bool| -> bool {
            (with && footprint.contains(&q))
                || self.map[q as usize].has_structure
                || self.landscape_info(self.map_get_landscape_type(q)).movement_speed[MOVEMENT_TRACKED as usize] == 0
        };
        // Size of the free region around `from` (stops at POCKET tiles, or at the map edge: open).
        let region = |from: u16, with: bool| -> (usize, bool) {
            let mut seen = vec![false; 4096];
            let mut stack = vec![from];
            seen[from as usize] = true;
            let mut n = 0;
            let mut has_unit = false;
            while let Some(p) = stack.pop() {
                n += 1;
                if n >= POCKET {
                    return (POCKET, has_unit);
                }
                has_unit |= self.unit_get_by_packed_tile(p).map_or(false, |u| self.ui(u).flags.is_ground_unit);
                let (x, y) = (packed_x(p) as i32, packed_y(p) as i32);
                // Ground next to one of the house's unit factories is where its units appear.
                if !has_unit {
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let (nx, ny) = (x + dx, y + dy);
                            if !(0..64).contains(&nx) || !(0..64).contains(&ny) {
                                continue;
                            }
                            if let Some(s) = self.structure_get_by_packed_tile(pack_xy(nx as u16, ny as u16)) {
                                let st = self.s(s);
                                has_unit |= st.o.house_id == h && st.o.type_ != STRUCTURE_CONSTRUCTION_YARD && self.si(s).o.flags.factory;
                            }
                        }
                    }
                }
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let (nx, ny) = (x + dx, y + dy);
                        if !(0..64).contains(&nx) || !(0..64).contains(&ny) {
                            continue;
                        }
                        let q = pack_xy(nx as u16, ny as u16);
                        if seen[q as usize] || !self.map_is_valid_position(q) || blocked(q, with) {
                            continue;
                        }
                        if dx != 0 && dy != 0 && blocked(pack_xy((x + dx) as u16, y as u16), with) && blocked(pack_xy(x as u16, (y + dy) as u16), with) {
                            continue;
                        }
                        seen[q as usize] = true;
                        stack.push(q);
                    }
                }
            }
            (n, has_unit)
        };
        for i in 0..16 {
            let off = STRUCTURE_LAYOUT_TILES_AROUND[layout][i];
            if off == 0 {
                break;
            }
            let q = position.wrapping_add(off as u16) & 0xFFF;
            if !self.map_is_valid_position(q) || blocked(q, true) {
                continue;
            }
            let (n, has_unit) = region(q, true);
            if n < POCKET && ((only_with_units && has_unit) || (!only_with_units && region(q, false).0 >= POCKET)) {
                return true;
            }
        }
        false
    }

    /// Skirmish addition: the heavy factory's pick (`structure_ai_pick_next_to_build`) is the unit with the
    /// highest build priority, i.e. a 600 credit siege tank or 800 credit devastator, and the computer's
    /// treasury is nearly always empty, so it builds one every hour while the cheap factories keep sending
    /// quads and infantry. Limit the choice to what the house can pay for now (a tank, 300, always counts);
    /// failing that, the cheapest.
    pub fn skirmish_ai_affordable(&self, h: u8, mask: u32) -> u32 {
        if !self.skirmish.enabled {
            return mask;
        }
        let budget = self.h(h).credits.max(300);
        let cost = |i: usize| self.unit_info[i].o.build_credits;
        let units: Vec<usize> = (0..UNIT_MAX).filter(|&i| mask & (1 << i) != 0).collect();
        let affordable = units.iter().filter(|&&i| cost(i) <= budget).fold(0u32, |m, &i| m | 1 << i);
        if affordable != 0 {
            return affordable;
        }
        units.iter().min_by_key(|&&i| cost(i)).map_or(mask, |&i| 1 << i)
    }

    /// Whether a computer heavy factory should build a harvester now.
    pub fn skirmish_ai_want_harvester(&self, h: u8) -> bool {
        if !self.skirmish.enabled || self.h(h).flags.human {
            return false;
        }
        let refineries = self.skirmish_count_structures(h, STRUCTURE_REFINERY);
        let harvesters = self.unit_find.iter().filter(|&&u| self.u(u).o.house_id == h && self.u(u).o.type_ == UNIT_HARVESTER).count() as u16;
        harvesters < refineries * 2 && harvesters < 6
    }

    /// Per-tick skirmish logic: AI credit storage grace, re-deploying stray MCVs, game end.
    pub fn skirmish_tick(&mut self) {
        if !self.skirmish.enabled || self.timer_game < self.skirmish.next_ai_tick {
            return;
        }
        self.skirmish.next_ai_tick = self.timer_game + 60;
        self.skirmish_worms_wander();
        self.skirmish_restart_harvesters();
        self.skirmish_unblock_units();
        self.skirmish_see_structures();
        self.skirmish_track_air_threat();
        for h in self.house_ids() {
            if self.h(h).flags.human {
                continue;
            }
            // Credits without storage, like g_playerCreditsNoSilo for the player.
            let ns = self.skirmish.ai_credits_no_silo[h as usize];
            if self.h(h).credits_storage > ns {
                self.skirmish.ai_credits_no_silo[h as usize] = 0;
            }
            // Computer MCVs deploy as soon as possible.
            for u in self.units_of(h, UNIT_MCV as u16) {
                if self.u(u).action_id != ACTION_DEPLOY && self.u(u).action_id != ACTION_MOVE {
                    self.unit_set_action(u, ACTION_DEPLOY);
                }
            }
        }
    }

    /// The credit cap for a computer house on the house tick (original: creditsStorage).
    pub fn skirmish_ai_credit_cap(&self, h: u8) -> u16 {
        if !self.skirmish.enabled {
            return self.h(h).credits_storage;
        }
        self.h(h).credits_storage.max(self.skirmish.ai_credits_no_silo[h as usize])
    }

    /// A house is still in the game while it has a structure (not counting walls, slabs,
    /// turrets) or an MCV.
    pub fn skirmish_house_alive(&self, h: u8) -> bool {
        // Only structures on the map, as Structure_Find gives GameLoop_IsLevelFinished: one that
        // was built but never placed (its construction yard destroyed first) does not count.
        let has_structure = self.structure_find.iter().any(|&s| {
            let st = self.s(s);
            st.o.house_id == h
                && !st.o.flags.is_not_on_map
                && !matches!(st.o.type_, STRUCTURE_SLAB_1X1 | STRUCTURE_SLAB_2X2 | STRUCTURE_WALL | STRUCTURE_TURRET | STRUCTURE_ROCKET_TURRET)
        });
        // An MCV on the map or being carried; not one still being built in a factory.
        has_structure
            || self.unit_find.iter().any(|&u| {
                let un = self.u(u);
                un.o.house_id == h
                    && un.o.type_ == UNIT_MCV
                    && (!un.o.flags.is_not_on_map || self.unit_find.iter().any(|&c| self.u(c).o.linked_id as u16 == u && self.u(c).o.flags.in_transport))
            })
    }

    /// GameLoop_LevelEnd for skirmish: Some(true) = won, Some(false) = lost.
    pub fn skirmish_check_end(&mut self) -> Option<bool> {
        if self.ticks.level_end_timer >= self.timer_game {
            return self.skirmish.finished;
        }
        self.ticks.level_end_timer = self.timer_game + 300;
        if self.skirmish.finished.is_some() {
            return self.skirmish.finished;
        }
        if self.timer_game - self.tick_scenario_start < 7200 {
            return None;
        }
        let p = self.player_house_id;
        let won = if !self.skirmish_house_alive(p) {
            false
        } else if self.house_ids().iter().all(|&h| h == p || h == HOUSE_FREMEN && self.skirmish.ai_allied || !self.skirmish_house_alive(h)) {
            true
        } else {
            return None;
        };
        // GameLoop_LevelEnd: drop any queued speech, then the win or lose voice. The UI does the
        // rest (music, message, stats, end window: see ui/end.rs).
        self.skirmish.finished = Some(won);
        self.feedback(0xFFFE);
        self.feedback(if won { 40 } else { 41 });
        self.skirmish.finished
    }

    /// Visibility for free-for-all: computer houses see each other (the original computer
    /// already always sees the player's units).
    pub fn ai_sees(&self, seen_by_houses: u8, house: u8) -> bool {
        if seen_by_houses & (1 << house) != 0 {
            return true;
        }
        self.skirmish.enabled && !self.skirmish.ai_allied && house != self.player_house_id
    }
}

#[allow(dead_code)]
fn unused(_: u16) {
    let _ = packed_y;
}
