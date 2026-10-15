//! The game simulation: a faithful port of Dune II's game logic (via OpenDUNE).
//!
//! All of the original's global state lives in [`Game`]. Functions keep the original names
//! (snake_cased) and structure so they can be compared against OpenDUNE side by side.

pub mod animation;
pub mod explosion;
pub mod house;
pub mod map;
pub mod pathfinder;
pub mod qol;
pub mod script;
pub mod script_fns;
pub mod skirmish;
pub mod structure;
pub mod team;
pub mod types;
pub mod unit;

use crate::data::strings::Strings;
use crate::map::Tile as MapTile;
use crate::rng::Rng;
use crate::tables::{self, StructureInfo, UnitInfo};
use crate::tile::{pack_xy, Tile32};
use script::ScriptInfo;
pub use types::*;

/// Things the simulation tells the front-end about (sounds, messages, ...).
#[derive(Clone, Debug)]
pub enum Event {
    /// Sound_Output_Feedback(index): voice + text + sound feedback for the player.
    Feedback(u16),
    /// Voice_PlayAtTile(voiceID, position).
    VoiceAtTile(u16, Tile32),
    /// Sound_StartSound(id).
    Sound(u16),
    /// GUI_DisplayText(text, priority).
    Text(Vec<u8>, i16),
    /// GUI_DisplayHint(stringID, spriteID).
    Hint(u16, u16),
    /// The radar was switched on/off (plays STATIC.WSA in the minimap).
    Radar(bool),
    /// A factory window must be shown for this structure (Structure_BuildObject(s, 0xFFFF)).
    FactoryWindow { structure: u16, construction_yard: bool, starport: bool, upgrade_cost: u16 },
    /// Selection type changed (GUI_ChangeSelectionType).
    SelectionType(SelectionType),
    /// Screen shake (Explosion_Func_ScreenShake).
    ScreenShake,
    /// Music request (Music_Play).
    Music(u16),
}

/// Formatting argument for the original printf-style text messages.
pub enum Arg<'a> {
    S(&'a [u8]),
    D(i32),
}

/// A tiny subset of printf, enough for the game's messages (%s, %d, %u, %%).
pub fn format_c(fmt: &[u8], args: &[Arg]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut a = args.iter();
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c == b'%' && i + 1 < fmt.len() {
            // skip flags/width
            let mut j = i + 1;
            while j < fmt.len() && (fmt[j].is_ascii_digit() || fmt[j] == b'-' || fmt[j] == b'l' || fmt[j] == b'h') {
                j += 1;
            }
            match fmt.get(j) {
                Some(b'%') => out.push(b'%'),
                Some(b's') => {
                    if let Some(Arg::S(s)) = a.next() {
                        out.extend_from_slice(s);
                    }
                }
                Some(b'd') | Some(b'u') | Some(b'i') => match a.next() {
                    Some(Arg::D(d)) => out.extend_from_slice(d.to_string().as_bytes()),
                    Some(Arg::S(s)) => out.extend_from_slice(s),
                    None => {}
                },
                _ => {}
            }
            i = j + 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Skirmish/game configuration that the original kept in g_gameConfig and friends.
#[derive(Clone, Debug)]
pub struct GameConfig {
    /// 0 = slowest .. 2 = normal .. 4 = fastest.
    pub game_speed: u16,
    /// OpenDUNE's "dune2_enhanced": fixes for outright bugs of the original.
    pub dune2_enhanced: bool,
    /// Quality of life (new; off = the original): the player's harvesters go back to their last
    /// field, or else to the nearest spice the player has seen (see qol.rs).
    pub harvester_return: bool,
    /// Quality of life (new; off = the original): repairing and upgrading a structure can go on
    /// at the same time, so neither cancels the other.
    pub repair_while_upgrading: bool,
    /// Quality of life (new; off = the original): a carryall that has held one of the player's units
    /// for a while with nowhere to take it is given somewhere (see qol.rs).
    pub carryall_timeout: bool,
    /// Simulation ticks per real tick, in percent (set by the UI's Turbo; 100 = real time).
    pub sim_percent: u32,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig { game_speed: 2, dune2_enhanced: true, harvester_return: false, repair_while_upgrading: false, carryall_timeout: false, sim_percent: 100 }
    }
}

pub struct Game {
    pub config: GameConfig,
    pub rng: Rng,
    pub strings: Strings,

    // Static tables (mutable copies, as the original modifies `available`).
    pub unit_info: [UnitInfo; UNIT_MAX],
    pub structure_info: [StructureInfo; STRUCTURE_MAX],

    // Tile set information needed by the logic.
    pub icon_map: Vec<u16>,
    pub veiled_tile_id: u16,
    pub bloom_tile_id: u16,
    pub landscape_tile_id: u16,
    pub built_slab_tile_id: u16,
    pub wall_tile_id: u16,

    // Scripts
    pub script_unit: ScriptInfo,
    pub script_structure: ScriptInfo,
    pub script_team: ScriptInfo,
    pub script_current_object: Option<ObjRef>,
    pub script_current_unit: Option<u16>,
    pub script_current_structure: Option<u16>,
    pub script_current_team: Option<u16>,

    // Pools
    pub units: Vec<Unit>,
    pub unit_find: Vec<u16>,
    pub structures: Vec<Structure>,
    pub structure_find: Vec<u16>,
    pub houses: Vec<House>,
    pub house_find: Vec<u8>,
    pub teams: Vec<Team>,
    pub team_find: Vec<u16>,
    pub explosions: Vec<Explosion>,
    pub animations: Vec<Animation>,

    // Map
    pub map: Vec<MapTile>,
    pub map_tile_id: Vec<u16>,

    // Timers
    pub timer_game: u32,
    pub timer_gui: u32,
    pub tick_scenario_start: u32,
    pub ticks: Ticks,

    // Globals
    pub scenario: Scenario,
    pub campaign_id: u16,
    pub scenario_id: u16,
    pub player_house_id: u8,
    pub player_credits_no_silo: u16,
    pub player_credits: u16,
    pub house_missile_countdown: u16,
    pub validate_strict_if_zero: u16,
    pub starport_available: [i16; UNIT_MAX],
    pub structure_index: u16,
    pub factory_window_construction_yard: bool,
    pub music_in_battle: i16,

    // UI state the logic touches
    pub unit_selected: Option<u16>,
    pub unit_active: Option<u16>,
    pub unit_house_missile: Option<u16>,
    pub structure_active: Option<u16>,
    pub structure_active_type: u16,
    pub structure_active_position: u16,
    pub active_action: u16,
    pub selection_type: SelectionType,
    pub selection_position: u16,
    pub selection_width: u16,
    pub selection_height: u16,
    pub selection_state: i16,
    pub selection_layout: u16,
    pub viewport_position: u16,
    pub minimap_position: u16,

    pub events: Vec<Event>,
    pub skirmish: skirmish::SkirmishState,
}

/// Per-subsystem "next tick" timers (the original's static s_tick* variables).
#[derive(Clone, Debug, Default)]
pub struct Ticks {
    pub unit_movement: u32,
    pub unit_rotation: u32,
    pub unit_blinking: u32,
    pub unit_unknown4: u32,
    pub unit_script: u32,
    pub unit_unknown5: u32,
    pub unit_deviation: u32,
    pub structure_degrade: u32,
    pub structure_structure: u32,
    pub structure_script: u32,
    pub structure_palace: u32,
    pub house_house: u32,
    pub house_power_maintenance: u32,
    pub house_starport: u32,
    pub house_reinforcement: u32,
    pub house_missile_countdown: u32,
    pub house_starport_availability: u32,
    pub team_game_loop: u32,
    pub explosion_timer: u32,
    pub animation_timer: u32,
    pub level_end_timer: u32,
    pub unit_status: u32,
}

impl Game {
    pub fn new(assets: &crate::data::Assets) -> Result<Game, String> {
        let t = &assets.tiles;
        let load = |name: &str| -> Result<ScriptInfo, String> {
            ScriptInfo::load(assets.vfs.get(name).ok_or_else(|| format!("missing {name}"))?)
        };
        let mut g = Game {
            config: GameConfig::default(),
            rng: Rng::new(),
            strings: Strings::load(&assets.vfs),
            unit_info: tables::UNIT_INFO,
            structure_info: tables::STRUCTURE_INFO,
            icon_map: t.icon_map.clone(),
            veiled_tile_id: t.veiled_tile(),
            bloom_tile_id: t.bloom_tile(),
            landscape_tile_id: t.landscape_tile(),
            built_slab_tile_id: t.built_slab_tile(),
            wall_tile_id: t.wall_tile(),
            script_unit: load("UNIT.EMC")?,
            script_structure: load("BUILD.EMC")?,
            script_team: load("TEAM.EMC")?,
            script_current_object: None,
            script_current_unit: None,
            script_current_structure: None,
            script_current_team: None,
            units: vec![Unit::default(); UNIT_INDEX_MAX],
            unit_find: Vec::new(),
            structures: vec![Structure::default(); STRUCTURE_INDEX_MAX_HARD],
            structure_find: Vec::new(),
            houses: vec![House::default(); HOUSE_INDEX_MAX],
            house_find: Vec::new(),
            teams: vec![Team::default(); TEAM_INDEX_MAX],
            team_find: Vec::new(),
            explosions: Vec::new(),
            animations: Vec::new(),
            map: vec![MapTile::default(); 4096],
            map_tile_id: vec![0; 4096],
            timer_game: 0,
            timer_gui: 0,
            tick_scenario_start: 0,
            ticks: Ticks::default(),
            scenario: Scenario::default(),
            campaign_id: 0,
            scenario_id: 0,
            player_house_id: HOUSE_INVALID,
            player_credits_no_silo: 0,
            player_credits: 0xFFFF,
            house_missile_countdown: 0,
            validate_strict_if_zero: 0,
            starport_available: [0; UNIT_MAX],
            structure_index: 0,
            factory_window_construction_yard: false,
            music_in_battle: 0,
            unit_selected: None,
            unit_active: None,
            unit_house_missile: None,
            structure_active: None,
            structure_active_type: 0xFFFF,
            structure_active_position: 0,
            active_action: 0xFFFF,
            selection_type: SelectionType::Mentat,
            selection_position: 0,
            selection_width: 1,
            selection_height: 1,
            selection_state: 0,
            selection_layout: 0,
            viewport_position: 0,
            minimap_position: 0,
            events: Vec::new(),
            skirmish: Default::default(),
        };
        g.game_init();
        Ok(g)
    }

    /// Game_Init
    pub fn game_init(&mut self) {
        self.units = vec![Unit::default(); UNIT_INDEX_MAX];
        self.unit_find.clear();
        self.structures = vec![Structure::default(); STRUCTURE_INDEX_MAX_HARD];
        self.structure_find.clear();
        self.teams = vec![Team::default(); TEAM_INDEX_MAX];
        self.team_find.clear();
        self.houses = vec![House::default(); HOUSE_INDEX_MAX];
        self.house_find.clear();
        self.animation_init();
        self.explosion_init();
        self.map = vec![MapTile::default(); 4096];
        self.map_tile_id = vec![0; 4096];
        self.starport_available = [0; UNIT_MAX];
        self.unit_info = tables::UNIT_INFO;
        self.structure_info = tables::STRUCTURE_INFO;
        self.player_credits_no_silo = 0;
        self.house_missile_countdown = 0;
        self.selection_state = 0;
        self.structure_active_position = 0;
        self.unit_house_missile = None;
        self.unit_active = None;
        self.unit_selected = None;
        self.structure_active = None;
        self.active_action = 0xFFFF;
        self.structure_active_type = 0xFFFF;
        self.scenario = Scenario::default();
        self.ticks = Ticks::default();
    }

    /// One 60 Hz tick: Timer_Tick followed by one in-game iteration of GameLoop_Main.
    pub fn tick(&mut self) {
        self.timer_game = self.timer_game.wrapping_add(1);
        self.timer_gui = self.timer_gui.wrapping_add(1);
        if !matches!(self.selection_type, SelectionType::Target | SelectionType::Place | SelectionType::Unit | SelectionType::Structure) {
            return;
        }
        if let Some(u) = self.unit_selected {
            if self.ticks.unit_status < self.timer_game {
                self.unit_display_status_text(u);
                self.ticks.unit_status = self.timer_game + 300;
            }
            if self.selection_type != SelectionType::Target {
                self.selection_position = self.u(u).o.position.center().pack();
            }
        }
        self.game_loop_team();
        self.game_loop_unit();
        self.game_loop_structure();
        self.game_loop_house();
        self.skirmish_tick();
        self.qol_carryall_tick();
        // GUI_DrawScreen
        self.explosion_tick();
        self.animation_tick();
        self.unit_sort();
        self.minimap_position = self.viewport_position;
        self.skirmish_check_end();
    }

    // ---------------------------------------------------------------------------------------
    // Messages / events
    // ---------------------------------------------------------------------------------------

    pub fn emit(&mut self, e: Event) {
        self.events.push(e);
    }

    pub fn string(&self, id: u16) -> Vec<u8> {
        self.strings.get(id).to_vec()
    }

    pub fn display_text(&mut self, text: Vec<u8>, priority: i16) {
        self.emit(Event::Text(text, priority));
    }

    pub fn display_string(&mut self, id: u16, priority: i16) {
        let t = self.string(id);
        self.display_text(t, priority);
    }

    pub fn feedback(&mut self, id: u16) {
        self.emit(Event::Feedback(id));
    }

    pub fn voice_at_tile(&mut self, id: u16, pos: Tile32) {
        self.emit(Event::VoiceAtTile(id, pos));
    }

    /// GUI_ChangeSelectionType (the parts that affect game state).
    pub fn change_selection_type(&mut self, t: SelectionType) {
        let mut t = t;
        if t == SelectionType::Unit && self.unit_selected.is_none() {
            t = SelectionType::Structure;
        }
        if t == SelectionType::Structure && self.unit_selected.is_some() {
            self.unit_selected = None;
        }
        if self.selection_type == t {
            return;
        }
        let old = self.selection_type;
        self.selection_type = t;
        match old {
            SelectionType::Place => {
                let p = self.structure_active_position;
                self.map_set_selection(p);
            }
            SelectionType::Unit => {
                if self.unit_selected.is_some() && t != SelectionType::Target && t != SelectionType::Unit {
                    self.unit_selected = None;
                }
            }
            _ => {}
        }
        match t {
            SelectionType::Target => self.structure_active_position = self.selection_position,
            SelectionType::Place => {
                self.unit_select(None);
                let layout = self.structure_info[self.structure_active_type as usize].layout;
                self.map_set_selection_size(layout);
            }
            _ => {}
        }
        self.emit(Event::SelectionType(t));
    }

    // ---------------------------------------------------------------------------------------
    // Tools
    // ---------------------------------------------------------------------------------------

    /// Tile_MoveByRandom using the game's RNG.
    pub fn tile_move_by_random(&mut self, tile: Tile32, distance: u16, center: bool) -> Tile32 {
        crate::tile::move_by_random(&mut self.rng, tile, distance, center)
    }

    /// Tools_AdjustToGameSpeed
    pub fn adjust_to_game_speed(&self, normal: u16, minimum: u16, maximum: u16, inverse_speed: bool) -> u16 {
        let mut game_speed = self.config.game_speed;
        if game_speed == 2 || game_speed > 4 {
            return normal;
        }
        let mut maximum = maximum;
        let mut minimum = minimum;
        if maximum as u32 > normal as u32 * 2 {
            maximum = normal.wrapping_mul(2);
        }
        if minimum < normal / 2 {
            minimum = normal / 2;
        }
        if inverse_speed {
            game_speed = 4 - game_speed;
        }
        match game_speed {
            0 => minimum,
            1 => normal - (normal - minimum) / 2,
            3 => normal + (maximum - normal) / 2,
            4 => maximum,
            _ => normal,
        }
    }

    pub fn index_get_type(encoded: u16) -> u16 {
        match encoded & 0xC000 {
            0x4000 => IT_UNIT,
            0x8000 => IT_STRUCTURE,
            0xC000 => IT_TILE,
            _ => IT_NONE,
        }
    }

    pub fn index_decode(encoded: u16) -> u16 {
        if Self::index_get_type(encoded) == IT_TILE {
            return pack_xy((encoded >> 1) & 0x3F, (encoded >> 8) & 0x3F);
        }
        encoded & 0x3FFF
    }

    pub fn index_encode(&self, index: u16, t: u16) -> u16 {
        match t {
            IT_TILE => {
                let mut ret = ((crate::tile::packed_x(index) << 1) + 1) << 0;
                ret |= ((crate::tile::packed_y(index) << 1) + 1) << 7;
                ret | 0xC000
            }
            IT_UNIT => {
                if index as usize >= UNIT_INDEX_MAX || !self.units[index as usize].o.flags.allocated {
                    return 0;
                }
                index | 0x4000
            }
            IT_STRUCTURE => index | 0x8000,
            _ => 0,
        }
    }

    pub fn index_is_valid(&self, encoded: u16) -> bool {
        if encoded == 0 {
            return false;
        }
        let index = Self::index_decode(encoded) as usize;
        match Self::index_get_type(encoded) {
            IT_UNIT => index < UNIT_INDEX_MAX && self.units[index].o.flags.used && self.units[index].o.flags.allocated,
            IT_STRUCTURE => index < STRUCTURE_INDEX_MAX_HARD && self.structures[index].o.flags.used,
            IT_TILE => true,
            _ => false,
        }
    }

    pub fn index_get_packed_tile(&self, encoded: u16) -> u16 {
        let index = Self::index_decode(encoded) as usize;
        match Self::index_get_type(encoded) {
            IT_TILE => index as u16,
            IT_UNIT => {
                if index < UNIT_INDEX_MAX {
                    self.units[index].o.position.pack()
                } else {
                    0
                }
            }
            IT_STRUCTURE => {
                if index < STRUCTURE_INDEX_MAX_HARD {
                    self.structures[index].o.position.pack()
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn index_get_tile(&self, encoded: u16) -> Tile32 {
        let index = Self::index_decode(encoded) as usize;
        match Self::index_get_type(encoded) {
            IT_TILE => Tile32::unpack(index as u16),
            IT_UNIT => {
                if index < UNIT_INDEX_MAX {
                    self.units[index].o.position
                } else {
                    Tile32::default()
                }
            }
            IT_STRUCTURE => {
                if index >= STRUCTURE_INDEX_MAX_HARD {
                    return Tile32::default();
                }
                let s = &self.structures[index];
                let si = &self.structure_info[s.o.type_ as usize];
                add_tile_diff(s.o.position, tables::STRUCTURE_LAYOUT_TILE_DIFF[si.layout as usize])
            }
            _ => Tile32::default(),
        }
    }

    pub fn index_get_unit(&self, encoded: u16) -> Option<u16> {
        if Self::index_get_type(encoded) != IT_UNIT {
            return None;
        }
        let index = Self::index_decode(encoded);
        ((index as usize) < UNIT_INDEX_MAX).then_some(index)
    }

    pub fn index_get_structure(&self, encoded: u16) -> Option<u16> {
        if Self::index_get_type(encoded) != IT_STRUCTURE {
            return None;
        }
        let index = Self::index_decode(encoded);
        ((index as usize) < STRUCTURE_INDEX_MAX_HARD).then_some(index)
    }

    pub fn index_get_object(&self, encoded: u16) -> Option<ObjRef> {
        match Self::index_get_type(encoded) {
            IT_UNIT => self.index_get_unit(encoded).map(ObjRef::Unit),
            IT_STRUCTURE => self.index_get_structure(encoded).map(ObjRef::Structure),
            _ => None,
        }
    }

    pub fn obj(&self, r: ObjRef) -> &Object {
        match r {
            ObjRef::Unit(i) => &self.units[i as usize].o,
            ObjRef::Structure(i) => &self.structures[i as usize].o,
        }
    }

    pub fn obj_mut(&mut self, r: ObjRef) -> &mut Object {
        match r {
            ObjRef::Unit(i) => &mut self.units[i as usize].o,
            ObjRef::Structure(i) => &mut self.structures[i as usize].o,
        }
    }

    pub fn unit_encode(&self, u: u16) -> u16 {
        self.index_encode(u, IT_UNIT)
    }

    pub fn structure_encode(&self, s: u16) -> u16 {
        self.index_encode(s, IT_STRUCTURE)
    }

    pub fn tile_encode(&self, packed: u16) -> u16 {
        self.index_encode(packed, IT_TILE)
    }

    pub fn u(&self, i: u16) -> &Unit {
        &self.units[i as usize]
    }

    pub fn um(&mut self, i: u16) -> &mut Unit {
        &mut self.units[i as usize]
    }

    pub fn s(&self, i: u16) -> &Structure {
        &self.structures[i as usize]
    }

    pub fn sm(&mut self, i: u16) -> &mut Structure {
        &mut self.structures[i as usize]
    }

    pub fn h(&self, i: u8) -> &House {
        &self.houses[i as usize]
    }

    pub fn hm(&mut self, i: u8) -> &mut House {
        &mut self.houses[i as usize]
    }

    pub fn ui(&self, u: u16) -> &UnitInfo {
        &self.unit_info[self.units[u as usize].o.type_ as usize]
    }

    pub fn si(&self, s: u16) -> &StructureInfo {
        &self.structure_info[self.structures[s as usize].o.type_ as usize]
    }

    // ---------------------------------------------------------------------------------------
    // Pools (pool/*.c)
    // ---------------------------------------------------------------------------------------

    /// Unit_Find
    pub fn unit_find(&self, find: &mut PoolFind) -> Option<u16> {
        let count = self.unit_find.len() as u16;
        if find.index >= count && find.index != 0xFFFF {
            return None;
        }
        find.index = find.index.wrapping_add(1);
        while find.index < self.unit_find.len() as u16 {
            let i = self.unit_find[find.index as usize];
            let u = &self.units[i as usize];
            let skip = (u.o.flags.is_not_on_map && self.validate_strict_if_zero == 0)
                || (find.house_id != HOUSE_INVALID && find.house_id != self.unit_get_house_id(i))
                || (find.type_ != 0xFFFF && find.type_ != u.o.type_ as u16);
            if !skip {
                return Some(i);
            }
            find.index += 1;
        }
        None
    }

    /// Unit_Allocate
    pub fn unit_allocate(&mut self, index: u16, type_: u8, house_id: u8) -> Option<u16> {
        if type_ == 0xFF || house_id == 0xFF {
            return None;
        }
        let h = &self.houses[house_id as usize];
        if h.unit_count >= h.unit_count_max {
            let mt = self.unit_info[type_ as usize].movement_type;
            if mt != MOVEMENT_WINGER && mt != MOVEMENT_SLITHER && self.validate_strict_if_zero == 0 {
                return None;
            }
        }
        let index = if index == 0 || index == INDEX_INVALID {
            let ui = &self.unit_info[type_ as usize];
            let found = (ui.index_start..=ui.index_end).find(|&i| !self.units[i as usize].o.flags.used)?;
            found
        } else {
            if self.units[index as usize].o.flags.used {
                return None;
            }
            index
        };
        self.houses[house_id as usize].unit_count += 1;
        let mut u = Unit::default();
        u.o.index = index;
        u.o.type_ = type_;
        u.o.house_id = house_id;
        u.o.linked_id = 0xFF;
        u.o.flags.used = true;
        u.o.flags.allocated = true;
        u.o.flags.is_unit = true;
        u.o.script.delay = 0;
        u.route[0] = 0xFF;
        if type_ == UNIT_SANDWORM {
            u.amount = 3;
        }
        self.units[index as usize] = u;
        self.unit_find.push(index);
        Some(index)
    }

    /// Unit_Free
    pub fn unit_free(&mut self, u: u16) {
        self.units[u as usize].o.flags = ObjectFlags::default();
        self.script_reset_obj(ObjRef::Unit(u), ScriptKind::Unit);
        if let Some(pos) = self.unit_find.iter().position(|&x| x == u) {
            self.unit_find.remove(pos);
            let h = self.units[u as usize].o.house_id;
            self.houses[h as usize].unit_count = self.houses[h as usize].unit_count.wrapping_sub(1);
        }
    }

    /// Structure_Find
    pub fn structure_find(&self, find: &mut PoolFind) -> Option<u16> {
        let count = self.structure_find.len() as u16;
        if find.index >= count + 3 && find.index != 0xFFFF {
            return None;
        }
        find.index = find.index.wrapping_add(1);
        while find.index < self.structure_find.len() as u16 + 3 {
            let count = self.structure_find.len() as u16;
            let idx = if find.index < count {
                Some(self.structure_find[find.index as usize])
            } else {
                let special = match find.index - count {
                    0 => STRUCTURE_INDEX_WALL,
                    1 => STRUCTURE_INDEX_SLAB_2X2,
                    _ => STRUCTURE_INDEX_SLAB_1X1,
                };
                (self.structures[special as usize].o.index == special).then_some(special)
            };
            if let Some(i) = idx {
                let s = &self.structures[i as usize];
                let skip = (s.o.flags.is_not_on_map && self.validate_strict_if_zero == 0)
                    || (find.house_id != HOUSE_INVALID && find.house_id != s.o.house_id)
                    || (find.type_ != 0xFFFF && find.type_ != s.o.type_ as u16);
                if !skip {
                    return Some(i);
                }
            }
            find.index += 1;
        }
        None
    }

    /// Structure_Allocate
    pub fn structure_allocate(&mut self, index: u16, type_: u8) -> Option<u16> {
        let index = match type_ {
            STRUCTURE_SLAB_1X1 => STRUCTURE_INDEX_SLAB_1X1,
            STRUCTURE_SLAB_2X2 => STRUCTURE_INDEX_SLAB_2X2,
            STRUCTURE_WALL => STRUCTURE_INDEX_WALL,
            _ => {
                let index = if index == INDEX_INVALID {
                    (0..STRUCTURE_INDEX_MAX_SOFT as u16).find(|&i| !self.structures[i as usize].o.flags.used)?
                } else {
                    if self.structures[index as usize].o.flags.used {
                        return None;
                    }
                    index
                };
                self.structure_find.push(index);
                index
            }
        };
        let mut s = Structure::default();
        s.o.index = index;
        s.o.type_ = type_;
        s.o.linked_id = 0xFF;
        s.o.flags.used = true;
        s.o.flags.allocated = true;
        s.o.script.delay = 0;
        self.structures[index as usize] = s;
        Some(index)
    }

    /// Structure_Free
    pub fn structure_free(&mut self, s: u16) {
        self.structures[s as usize].o.flags = ObjectFlags::default();
        self.script_reset_obj(ObjRef::Structure(s), ScriptKind::Structure);
        let t = self.structures[s as usize].o.type_;
        if t == STRUCTURE_SLAB_1X1 || t == STRUCTURE_SLAB_2X2 || t == STRUCTURE_WALL {
            return;
        }
        if let Some(pos) = self.structure_find.iter().position(|&x| x == s) {
            self.structure_find.remove(pos);
        }
    }

    /// Structure_Recount
    pub fn structure_recount(&mut self) {
        for &h in &self.house_find.clone() {
            self.houses[h as usize].unit_count = 0;
        }
        self.structure_find = (0..STRUCTURE_INDEX_MAX_SOFT as u16).filter(|&i| self.structures[i as usize].o.flags.used).collect();
    }

    /// Unit_Recount
    pub fn unit_recount(&mut self) {
        for &h in &self.house_find.clone() {
            self.houses[h as usize].unit_count = 0;
        }
        self.unit_find.clear();
        for i in 0..UNIT_INDEX_MAX as u16 {
            if !self.units[i as usize].o.flags.used {
                continue;
            }
            let h = self.units[i as usize].o.house_id;
            self.houses[h as usize].unit_count += 1;
            self.unit_find.push(i);
        }
    }

    /// House_Find
    pub fn house_find(&self, find: &mut PoolFind) -> Option<u8> {
        let count = self.house_find.len() as u16;
        if find.index >= count && find.index != 0xFFFF {
            return None;
        }
        find.index = find.index.wrapping_add(1);
        if find.index < self.house_find.len() as u16 {
            return Some(self.house_find[find.index as usize]);
        }
        None
    }

    /// All allocated houses, in pool order.
    pub fn house_ids(&self) -> Vec<u8> {
        self.house_find.clone()
    }

    /// House_Allocate
    pub fn house_allocate(&mut self, index: u8) -> Option<u8> {
        if index as usize >= HOUSE_INDEX_MAX || self.houses[index as usize].flags.used {
            return None;
        }
        let h = House { index, flags: HouseFlags { used: true, ..Default::default() }, starport_linked_id: INDEX_INVALID, ..Default::default() };
        self.houses[index as usize] = h;
        self.house_find.push(index);
        Some(index)
    }

    /// Team_Find
    pub fn team_find(&self, find: &mut PoolFind) -> Option<u16> {
        let count = self.team_find.len() as u16;
        if find.index >= count && find.index != 0xFFFF {
            return None;
        }
        find.index = find.index.wrapping_add(1);
        while find.index < self.team_find.len() as u16 {
            let t = self.team_find[find.index as usize];
            if find.house_id == HOUSE_INVALID || find.house_id == self.teams[t as usize].house_id {
                return Some(t);
            }
            find.index += 1;
        }
        None
    }

    /// Team_Allocate
    pub fn team_allocate(&mut self, index: u16) -> Option<u16> {
        let index = if index == INDEX_INVALID {
            (0..TEAM_INDEX_MAX as u16).find(|&i| !self.teams[i as usize].used)?
        } else {
            if self.teams[index as usize].used {
                return None;
            }
            index
        };
        self.teams[index as usize] = Team { index, used: true, ..Default::default() };
        self.team_find.push(index);
        Some(index)
    }

    /// Team_Recount
    pub fn team_recount(&mut self) {
        self.team_find = (0..TEAM_INDEX_MAX as u16).filter(|&i| self.teams[i as usize].used).collect();
    }

    /// Collect the results of a pool search up-front (only safe where the loop body cannot
    /// change the pool; otherwise iterate with `unit_find` directly).
    pub fn units_of(&self, house_id: u8, type_: u16) -> Vec<u16> {
        let mut f = PoolFind::new(house_id, type_);
        let mut v = Vec::new();
        while let Some(u) = self.unit_find(&mut f) {
            v.push(u);
        }
        v
    }

    pub fn structures_of(&self, house_id: u8, type_: u16) -> Vec<u16> {
        let mut f = PoolFind::new(house_id, type_);
        let mut v = Vec::new();
        while let Some(s) = self.structure_find(&mut f) {
            v.push(s);
        }
        v
    }

    // ---------------------------------------------------------------------------------------
    // Object (object.c)
    // ---------------------------------------------------------------------------------------

    /// Object_Script_Variable4_Link
    pub fn object_script_variable4_link(&mut self, encoded_from: u16, encoded_to: u16) {
        if !self.index_is_valid(encoded_from) || !self.index_is_valid(encoded_to) {
            return;
        }
        let (Some(from), Some(to)) = (self.index_get_object(encoded_from), self.index_get_object(encoded_to)) else { return };
        if self.obj(from).script.variables[4] != self.obj(to).script.variables[4] {
            self.object_script_variable4_clear(from);
            self.object_script_variable4_clear(to);
        }
        if self.obj(from).script.variables[4] != 0 {
            return;
        }
        self.object_script_variable4_set(Some(from), encoded_to);
        self.object_script_variable4_set(Some(to), encoded_from);
    }

    /// Object_Script_Variable4_Set
    pub fn object_script_variable4_set(&mut self, o: Option<ObjRef>, encoded: u16) {
        let Some(o) = o else { return };
        self.obj_mut(o).script.variables[4] = encoded;
        let ObjRef::Structure(s) = o else { return };
        if self.structures[s as usize].o.flags.is_unit {
            return;
        }
        if !self.si(s).o.flags.busy_state_is_incoming {
            return;
        }
        if self.structures[s as usize].o.linked_id != 0xFF {
            return;
        }
        self.structure_set_state(s, if encoded == 0 { STRUCTURE_STATE_IDLE } else { STRUCTURE_STATE_BUSY });
    }

    /// Object_Script_Variable4_Clear
    pub fn object_script_variable4_clear(&mut self, o: ObjRef) {
        let encoded = self.obj(o).script.variables[4];
        if encoded == 0 {
            return;
        }
        let other = self.index_get_object(encoded);
        self.object_script_variable4_set(Some(o), 0);
        self.object_script_variable4_set(other, 0);
    }

    /// Object_GetByPackedTile
    pub fn object_get_by_packed_tile(&self, packed: u16) -> Option<ObjRef> {
        if crate::tile::is_out_of_map(packed) {
            return None;
        }
        let t = &self.map[packed as usize];
        if t.has_unit {
            return Some(ObjRef::Unit(t.index as u16 - 1));
        }
        if t.has_structure {
            return Some(ObjRef::Structure(t.index as u16 - 1));
        }
        None
    }

    /// Object_GetDistanceToEncoded
    pub fn object_get_distance_to_encoded(&self, o: ObjRef, encoded: u16) -> u16 {
        let opos = self.obj(o).position;
        let position = if let Some(s) = self.index_get_structure(encoded) {
            let spos = self.structures[s as usize].o.position;
            let mut packed = spos.pack();
            let layout = self.si(s).layout as usize;
            let dir = orientation_256_to_8(get_direction(opos, spos) as u8);
            packed = packed.wrapping_add(tables::STRUCTURE_LAYOUT_EDGE_TILES[layout][((dir + 4) & 7) as usize]);
            Tile32::unpack(packed)
        } else {
            self.index_get_tile(encoded)
        };
        get_distance(opos, position)
    }

    /// House_AreAllied
    pub fn house_are_allied(&self, h1: u8, h2: u8) -> bool {
        if h1 == HOUSE_INVALID || h2 == HOUSE_INVALID {
            return false;
        }
        if h1 == h2 {
            return true;
        }
        if h1 == HOUSE_FREMEN || h2 == HOUSE_FREMEN {
            // The original has the Fremen (and so their worms) allied with Atreides, but a worm eats
            // anything on the sand, Atreides included: in skirmish they are nobody's friend, so they can be
            // attacked and shot at by everyone.
            return !self.skirmish.enabled && (h1 == HOUSE_ATREIDES || h2 == HOUSE_ATREIDES);
        }
        if self.skirmish.enabled && !self.skirmish.ai_allied {
            return false;
        }
        h1 != self.player_house_id && h2 != self.player_house_id
    }
}

// -------------------------------------------------------------------------------------------
// Tile math that depends on nothing but positions (tile.c)
// -------------------------------------------------------------------------------------------

/// Tile_GetDistance
pub fn get_distance(from: Tile32, to: Tile32) -> u16 {
    let dx = (from.x as i32 - to.x as i32).unsigned_abs() as u16;
    let dy = (from.y as i32 - to.y as i32).unsigned_abs() as u16;
    if dx > dy {
        dx.wrapping_add(dy / 2)
    } else {
        dy.wrapping_add(dx / 2)
    }
}

/// Tile_AddTileDiff
pub fn add_tile_diff(from: Tile32, diff: Tile32) -> Tile32 {
    Tile32 { x: from.x.wrapping_add(diff.x), y: from.y.wrapping_add(diff.y) }
}

/// Tile_GetDistancePacked
pub fn get_distance_packed(from: u16, to: u16) -> u16 {
    get_distance(Tile32::unpack(from), Tile32::unpack(to)) >> 8
}

/// Tile_GetDistanceRoundedUp
pub fn get_distance_rounded_up(from: Tile32, to: Tile32) -> u16 {
    ((get_distance(from, to) as u32 + 0x80) >> 8) as u16
}

/// Tile_GetDirectionPacked
pub fn get_direction_packed(from: u16, to: u16) -> u8 {
    const RET: [u8; 16] = [0x20, 0x40, 0x20, 0x00, 0xE0, 0xC0, 0xE0, 0x00, 0x60, 0x40, 0x60, 0x80, 0xA0, 0xC0, 0xA0, 0x80];
    let (x1, y1) = (crate::tile::packed_x(from) as i16, crate::tile::packed_y(from) as i16);
    let (x2, y2) = (crate::tile::packed_x(to) as i16, crate::tile::packed_y(to) as i16);
    let mut index = 0usize;
    let mut dy = y1 - y2;
    if dy < 0 {
        index |= 0x8;
        dy = -dy;
    }
    let mut dx = x2 - x1;
    if dx < 0 {
        index |= 0x4;
        dx = -dx;
    }
    if dx >= dy {
        if (dx + 1) / 2 > dy {
            index |= 0x1;
        }
    } else {
        index |= 0x2;
        if (dy + 1) / 2 > dx {
            index |= 0x1;
        }
    }
    RET[index]
}

/// Tile_MoveByDirection
pub fn move_by_direction(tile: Tile32, orientation: i16, distance: u16) -> Tile32 {
    let distance = distance.min(0xFF) as i32;
    if distance == 0 {
        return tile;
    }
    let dx = crate::tile::STEPX[(orientation & 0xFF) as usize] as i32;
    let dy = crate::tile::STEPY[(orientation & 0xFF) as usize] as i32;
    let rx = if dx < 0 { -64 } else { 64 };
    let ry = if dy < 0 { -64 } else { 64 };
    Tile32 {
        x: tile.x.wrapping_add(((dx * distance + rx) / 128) as u16),
        y: tile.y.wrapping_sub(((dy * distance + ry) / 128) as u16),
    }
}

/// Tile_GetDirection
pub fn get_direction(from: Tile32, to: Tile32) -> i8 {
    const ORIENTATION_OFFSETS: [u16; 4] = [0x40, 0x80, 0x0, 0xC0];
    const DIRECTIONS: [i32; 32] = [
        0x3FFF, 0x28BC, 0x145A, 0xD8E, 0xA27, 0x81B, 0x6BD, 0x5C3, 0x506, 0x474, 0x3FE, 0x39D, 0x34B, 0x306, 0x2CB, 0x297,
        0x26A, 0x241, 0x21D, 0x1FC, 0x1DE, 0x1C3, 0x1AB, 0x194, 0x17F, 0x16B, 0x159, 0x148, 0x137, 0x128, 0x11A, 0x10C,
    ];
    let mut dx = to.x as i32 - from.x as i32;
    let mut dy = to.y as i32 - from.y as i32;
    if dx.abs() + dy.abs() > 8000 {
        dx /= 2;
        dy /= 2;
    }
    let mut quadrant = 0usize;
    if dy <= 0 {
        quadrant |= 0x2;
        dy = -dy;
    }
    if dx < 0 {
        quadrant |= 0x1;
        dx = -dx;
    }
    let base = ORIENTATION_OFFSETS[quadrant];
    let mut invert = false;
    let mut gradient = 0x7FFF;
    if dx >= dy {
        if dy != 0 {
            gradient = (dx << 8) / dy;
        }
    } else {
        invert = true;
        if dx != 0 {
            gradient = (dy << 8) / dx;
        }
    }
    let mut i = DIRECTIONS.iter().position(|&d| d <= gradient).unwrap_or(32) as u16;
    if !invert {
        i = 64 - i;
    }
    if quadrant == 0 || quadrant == 3 {
        return ((base + 64 - i) & 0xFF) as u8 as i8;
    }
    ((base + i) & 0xFF) as u8 as i8
}

/// Tile_MoveByOrientation
pub fn move_by_orientation(position: Tile32, orientation: u8) -> Tile32 {
    const XO: [u16; 8] = [0, 256, 256, 256, 0, 0u16.wrapping_sub(256), 0u16.wrapping_sub(256), 0u16.wrapping_sub(256)];
    const YO: [u16; 8] = [0u16.wrapping_sub(256), 0u16.wrapping_sub(256), 0, 256, 256, 256, 0, 0u16.wrapping_sub(256)];
    let o = orientation_256_to_8(orientation) as usize;
    let x = position.x.wrapping_add(XO[o]);
    let y = position.y.wrapping_add(YO[o]);
    if x > 16384 || y > 16384 {
        return position;
    }
    Tile32 { x, y }
}

pub fn orientation_256_to_8(o: u8) -> u8 {
    (((o as u16 + 16) / 32) & 0x7) as u8
}

pub fn orientation_256_to_16(o: u8) -> u8 {
    (((o as u16 + 8) / 16) & 0xF) as u8
}
