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
use crate::tables::landscape::LANDSCAPE_INFO;
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

        self.tick_scenario_start = self.timer_game;
        self.validate_strict_if_zero -= 1;
        self.game_prepare();

        let p = starts[0];
        self.map_set_viewport_position(p);
        self.minimap_position = self.viewport_position;
        self.selection_position = p;
        self.selection_type = SelectionType::Structure;
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
    pub fn skirmish_drop_blocked_rebuilds(&mut self, h: u8) {
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
            }) || self.skirmish_footprint_has_units(t as u8, position);
            if blocked {
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
        if !self.skirmish.enabled {
            return false;
        }
        let layout = self.structure_info[t as usize].layout as usize;
        let footprint: Vec<u16> =
            (0..STRUCTURE_LAYOUT_TILE_COUNT[layout] as usize).map(|j| position.wrapping_add(STRUCTURE_LAYOUT_TILES[layout][j]) & 0xFFF).collect();
        self.unit_find.iter().any(|&u| {
            let un = self.u(u);
            if un.o.flags.is_not_on_map || !self.ui(u).flags.is_ground_unit {
                return false;
            }
            let dest = un.current_destination;
            footprint.contains(&un.o.position.pack()) || (dest.x != 0 || dest.y != 0) && footprint.contains(&dest.pack())
        })
    }

    pub fn skirmish_ai_place_structure(&mut self, h: u8, s: u16) -> bool {
        if !self.skirmish.enabled {
            return false;
        }
        let t = self.s(s).o.type_;
        let layout = self.structure_info[t as usize].layout as usize;
        let count = STRUCTURE_LAYOUT_TILE_COUNT[layout] as u16;
        // Centre of the base: the first construction yard, else the start position.
        let mut find = PoolFind::new(h, STRUCTURE_CONSTRUCTION_YARD as u16);
        let center = match self.structure_find(&mut find) {
            Some(cy) => self.s(cy).o.position.pack() + 65,
            None => self.skirmish.start_positions.iter().find(|(hh, _)| *hh == h).map(|x| x.1).unwrap_or(0),
        };
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
                    if self.structure_is_valid_build_location_for(p, t, h) == 0 || self.skirmish_footprint_has_units(t, p) {
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
                    let score = if is_defense { -dist + blocked * 3 } else { dist + blocked * 6 + count as i32 };
                    if best.map_or(true, |(_, b)| score < b) {
                        best = Some((p, score));
                    }
                }
            }
            if !is_defense && best.is_some() && r > 3 {
                break;
            }
        }
        let Some((p, _)) = best else { return false };
        self.validate_strict_if_zero += 1;
        let ok = self.structure_place(s, p);
        self.validate_strict_if_zero -= 1;
        ok
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
