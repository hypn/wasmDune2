//! Core game object types: Object, Unit, Structure, House, Team, ScriptEngine.

use crate::tile::Tile32;

pub const UNIT_INDEX_MAX: usize = 102;
pub const STRUCTURE_INDEX_MAX_SOFT: usize = 79;
pub const STRUCTURE_INDEX_MAX_HARD: usize = 82;
pub const STRUCTURE_INDEX_WALL: u16 = 79;
pub const STRUCTURE_INDEX_SLAB_2X2: u16 = 80;
pub const STRUCTURE_INDEX_SLAB_1X1: u16 = 81;
pub const HOUSE_INDEX_MAX: usize = 6;
pub const TEAM_INDEX_MAX: usize = 16;
pub const INDEX_INVALID: u16 = 0xFFFF;

pub const HOUSE_HARKONNEN: u8 = 0;
pub const HOUSE_ATREIDES: u8 = 1;
pub const HOUSE_ORDOS: u8 = 2;
pub const HOUSE_FREMEN: u8 = 3;
pub const HOUSE_SARDAUKAR: u8 = 4;
pub const HOUSE_MERCENARY: u8 = 5;
pub const HOUSE_MAX: u8 = 6;
pub const HOUSE_INVALID: u8 = 0xFF;

pub const HOUSE_WEAPON_MISSILE: u16 = 1;
pub const HOUSE_WEAPON_FREMEN: u16 = 2;
pub const HOUSE_WEAPON_SABOTEUR: u16 = 3;

// Unit types
pub const UNIT_CARRYALL: u8 = 0;
pub const UNIT_ORNITHOPTER: u8 = 1;
pub const UNIT_INFANTRY: u8 = 2;
pub const UNIT_TROOPERS: u8 = 3;
pub const UNIT_SOLDIER: u8 = 4;
pub const UNIT_TROOPER: u8 = 5;
pub const UNIT_SABOTEUR: u8 = 6;
pub const UNIT_LAUNCHER: u8 = 7;
pub const UNIT_DEVIATOR: u8 = 8;
pub const UNIT_TANK: u8 = 9;
pub const UNIT_SIEGE_TANK: u8 = 10;
pub const UNIT_DEVASTATOR: u8 = 11;
pub const UNIT_SONIC_TANK: u8 = 12;
pub const UNIT_TRIKE: u8 = 13;
pub const UNIT_RAIDER_TRIKE: u8 = 14;
pub const UNIT_QUAD: u8 = 15;
pub const UNIT_HARVESTER: u8 = 16;
pub const UNIT_MCV: u8 = 17;
pub const UNIT_MISSILE_HOUSE: u8 = 18;
pub const UNIT_MISSILE_ROCKET: u8 = 19;
pub const UNIT_MISSILE_TURRET: u8 = 20;
pub const UNIT_MISSILE_DEVIATOR: u8 = 21;
pub const UNIT_MISSILE_TROOPER: u8 = 22;
pub const UNIT_BULLET: u8 = 23;
pub const UNIT_SONIC_BLAST: u8 = 24;
pub const UNIT_SANDWORM: u8 = 25;
pub const UNIT_FRIGATE: u8 = 26;
pub const UNIT_MAX: usize = 27;
pub const UNIT_INVALID: u8 = 0xFF;

// Structure types
pub const STRUCTURE_SLAB_1X1: u8 = 0;
pub const STRUCTURE_SLAB_2X2: u8 = 1;
pub const STRUCTURE_PALACE: u8 = 2;
pub const STRUCTURE_LIGHT_VEHICLE: u8 = 3;
pub const STRUCTURE_HEAVY_VEHICLE: u8 = 4;
pub const STRUCTURE_HIGH_TECH: u8 = 5;
pub const STRUCTURE_HOUSE_OF_IX: u8 = 6;
pub const STRUCTURE_WOR_TROOPER: u8 = 7;
pub const STRUCTURE_CONSTRUCTION_YARD: u8 = 8;
pub const STRUCTURE_WINDTRAP: u8 = 9;
pub const STRUCTURE_BARRACKS: u8 = 10;
pub const STRUCTURE_STARPORT: u8 = 11;
pub const STRUCTURE_REFINERY: u8 = 12;
pub const STRUCTURE_REPAIR: u8 = 13;
pub const STRUCTURE_WALL: u8 = 14;
pub const STRUCTURE_TURRET: u8 = 15;
pub const STRUCTURE_ROCKET_TURRET: u8 = 16;
pub const STRUCTURE_SILO: u8 = 17;
pub const STRUCTURE_OUTPOST: u8 = 18;
pub const STRUCTURE_MAX: usize = 19;
pub const STRUCTURE_INVALID: u8 = 0xFF;

pub const STRUCTURE_STATE_DETECT: i16 = -2;
pub const STRUCTURE_STATE_JUSTBUILT: i16 = -1;
pub const STRUCTURE_STATE_IDLE: i16 = 0;
pub const STRUCTURE_STATE_BUSY: i16 = 1;
pub const STRUCTURE_STATE_READY: i16 = 2;

// Actions
pub const ACTION_ATTACK: u8 = 0;
pub const ACTION_MOVE: u8 = 1;
pub const ACTION_RETREAT: u8 = 2;
pub const ACTION_GUARD: u8 = 3;
pub const ACTION_AREA_GUARD: u8 = 4;
pub const ACTION_HARVEST: u8 = 5;
pub const ACTION_RETURN: u8 = 6;
pub const ACTION_STOP: u8 = 7;
pub const ACTION_AMBUSH: u8 = 8;
pub const ACTION_SABOTAGE: u8 = 9;
pub const ACTION_DIE: u8 = 10;
pub const ACTION_HUNT: u8 = 11;
pub const ACTION_DEPLOY: u8 = 12;
pub const ACTION_DESTRUCT: u8 = 13;
pub const ACTION_MAX: u8 = 14;
pub const ACTION_INVALID: u8 = 0xFF;

// Movement types
pub const MOVEMENT_FOOT: u16 = 0;
pub const MOVEMENT_TRACKED: u16 = 1;
pub const MOVEMENT_HARVESTER: u16 = 2;
pub const MOVEMENT_WHEELED: u16 = 3;
pub const MOVEMENT_WINGER: u16 = 4;
pub const MOVEMENT_SLITHER: u16 = 5;

// Display modes
pub const DISPLAYMODE_SINGLE_FRAME: u16 = 0;
pub const DISPLAYMODE_UNIT: u16 = 1;
pub const DISPLAYMODE_ROCKET: u16 = 2;
pub const DISPLAYMODE_INFANTRY_3_FRAMES: u16 = 3;
pub const DISPLAYMODE_INFANTRY_4_FRAMES: u16 = 4;
pub const DISPLAYMODE_ORNITHOPTER: u16 = 5;

// Team actions
pub const TEAM_ACTION_NORMAL: u16 = 0;
pub const TEAM_ACTION_STAGING: u16 = 1;
pub const TEAM_ACTION_FLEE: u16 = 2;
pub const TEAM_ACTION_KAMIKAZE: u16 = 3;
pub const TEAM_ACTION_GUARD: u16 = 4;

// Explosion types
pub const EXPLOSION_IMPACT_SMALL: u16 = 0;
pub const EXPLOSION_IMPACT_MEDIUM: u16 = 1;
pub const EXPLOSION_IMPACT_LARGE: u16 = 2;
pub const EXPLOSION_IMPACT_EXPLODE: u16 = 3;
pub const EXPLOSION_SABOTEUR_DEATH: u16 = 4;
pub const EXPLOSION_SABOTEUR_INFILTRATE: u16 = 5;
pub const EXPLOSION_TANK_EXPLODE: u16 = 6;
pub const EXPLOSION_DEVIATOR_GAS: u16 = 7;
pub const EXPLOSION_SAND_BURST: u16 = 8;
pub const EXPLOSION_TANK_FLAMES: u16 = 9;
pub const EXPLOSION_WHEELED_VEHICLE: u16 = 10;
pub const EXPLOSION_DEATH_HAND: u16 = 11;
pub const EXPLOSION_UNUSED_12: u16 = 12;
pub const EXPLOSION_SANDWORM_SWALLOW: u16 = 13;
pub const EXPLOSION_STRUCTURE: u16 = 14;
pub const EXPLOSION_SMOKE_PLUME: u16 = 15;
pub const EXPLOSION_ORNITHOPTER_CRASH: u16 = 16;
pub const EXPLOSION_CARRYALL_CRASH: u16 = 17;
pub const EXPLOSION_MINI_ROCKET: u16 = 18;
pub const EXPLOSION_SPICE_BLOOM_TREMOR: u16 = 19;

// Index types (Tools_Index_GetType)
pub const IT_NONE: u16 = 0;
pub const IT_TILE: u16 = 1;
pub const IT_UNIT: u16 = 2;
pub const IT_STRUCTURE: u16 = 3;

/// Which compiled script (EMC) an engine executes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScriptKind {
    #[default]
    None,
    Unit,
    Structure,
    Team,
}

/// ScriptEngine: the per-object EMC virtual machine state.
#[derive(Clone, Debug, Default)]
pub struct ScriptEngine {
    pub delay: u16,
    /// Instruction pointer as word offset into the script DATA, or None when not loaded.
    pub script: Option<u16>,
    pub info: ScriptKind,
    pub return_value: u16,
    pub frame_pointer: u8,
    pub stack_pointer: u8,
    pub variables: [u16; 5],
    pub stack: [u16; 15],
    pub is_subroutine: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ObjectFlags {
    pub used: bool,
    pub allocated: bool,
    pub is_not_on_map: bool,
    pub is_smoking: bool,
    pub fire_twice_flip: bool,
    pub animation_flip: bool,
    pub bullet_is_big: bool,
    pub is_wobbling: bool,
    pub in_transport: bool,
    pub by_scenario: bool,
    pub degrades: bool,
    pub is_highlighted: bool,
    pub is_dirty: bool,
    pub repairing: bool,
    pub on_hold: bool,
    pub is_unit: bool,
    pub upgrading: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Object {
    pub index: u16,
    pub type_: u8,
    pub linked_id: u8,
    pub flags: ObjectFlags,
    pub house_id: u8,
    pub seen_by_houses: u8,
    pub position: Tile32,
    pub hitpoints: u16,
    pub script: ScriptEngine,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Dir24 {
    pub speed: i8,
    pub target: i8,
    pub current: i8,
}

#[derive(Clone, Debug, Default)]
pub struct Unit {
    pub o: Object,
    pub current_destination: Tile32,
    pub origin_encoded: u16,
    pub action_id: u8,
    pub next_action_id: u8,
    pub fire_delay: u16,
    pub distance_to_destination: u16,
    pub target_attack: u16,
    pub target_move: u16,
    pub amount: u8,
    pub deviated: u8,
    pub deviated_house: u8,
    pub target_last: Tile32,
    pub target_pre_last: Tile32,
    pub orientation: [Dir24; 2],
    pub speed_per_tick: u8,
    pub speed_remainder: u8,
    pub speed: u8,
    pub moving_speed: u8,
    pub wobble_index: u8,
    pub sprite_offset: i8,
    pub blink_counter: u8,
    pub team: u8,
    pub timer: u16,
    pub route: [u8; 14],
    /// New (not in the original): the tile a harvester last took spice from, for
    /// `GameConfig::harvester_return`. 0 = none.
    pub last_harvest: u16,
    /// New: seconds a carryall has held a unit with no destination, for `GameConfig::carryall_timeout`.
    pub carry_idle: u8,
    /// New: a tile (encoded) `su_move_to_structure` hands a stuck carryall in place of a refinery.
    pub carry_drop: u16,
    /// New: seconds a computer ground unit with somewhere to go has not moved (see `skirmish_unblock_units`).
    pub stuck_secs: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Structure {
    pub o: Object,
    pub creator_house_id: u16,
    pub rotation_sprite_diff: u16,
    pub object_type: u16,
    pub upgrade_level: u8,
    pub upgrade_time_left: u8,
    pub count_down: u16,
    pub build_cost_remainder: u16,
    pub state: i16,
    pub hitpoints_max: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HouseFlags {
    pub used: bool,
    pub human: bool,
    pub done_full_scale_attack: bool,
    pub is_ai_active: bool,
    pub radar_activated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct House {
    pub index: u8,
    pub harvesters_incoming: u16,
    pub flags: HouseFlags,
    pub unit_count: u16,
    pub unit_count_max: u16,
    pub unit_count_enemy: u16,
    pub unit_count_allied: u16,
    pub structures_built: u32,
    pub credits: u16,
    pub credits_storage: u16,
    pub power_production: u16,
    pub power_usage: u16,
    pub windtrap_count: u16,
    pub credits_quota: u16,
    pub palace_position: Tile32,
    pub timer_unit_attack: u16,
    pub timer_sandworm_attack: u16,
    pub timer_structure_attack: u16,
    pub starport_time_left: u16,
    pub starport_linked_id: u16,
    pub ai_structure_rebuild: [[u16; 2]; 5],
}

#[derive(Clone, Debug, Default)]
pub struct Team {
    pub index: u16,
    pub used: bool,
    pub members: u16,
    pub min_members: u16,
    pub max_members: u16,
    pub movement_type: u16,
    pub action: u16,
    pub action_start: u16,
    pub house_id: u8,
    pub position: Tile32,
    pub target_tile: u16,
    pub target: u16,
    pub script: ScriptEngine,
}

/// PoolFindStruct
#[derive(Clone, Copy, Debug)]
pub struct PoolFind {
    pub house_id: u8,
    pub type_: u16,
    pub index: u16,
}

impl PoolFind {
    pub fn new(house_id: u8, type_: u16) -> Self {
        PoolFind { house_id, type_, index: 0xFFFF }
    }
    pub fn all() -> Self {
        Self::new(HOUSE_INVALID, 0xFFFF)
    }
}

/// A reference to either a Unit or a Structure (the original's `Object *`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjRef {
    Unit(u16),
    Structure(u16),
}

#[derive(Clone, Copy, Debug)]
pub struct ReinforcementData {
    pub unit_id: u16,
    pub location_id: u16,
    pub time_left: u16,
    pub time_between: u16,
    pub repeat: u16,
}

impl Default for ReinforcementData {
    fn default() -> Self {
        ReinforcementData { unit_id: INDEX_INVALID, location_id: 0, time_left: 0, time_between: 0, repeat: 0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scenario {
    pub score: u16,
    pub win_flags: u16,
    pub lose_flags: u16,
    pub map_seed: u32,
    pub map_scale: u16,
    pub time_out: u16,
    pub killed_allied: u16,
    pub killed_enemy: u16,
    pub destroyed_allied: u16,
    pub destroyed_enemy: u16,
    pub harvested_allied: u16,
    pub harvested_enemy: u16,
    pub reinforcement: [ReinforcementData; 16],
}

#[derive(Clone, Copy, Debug)]
pub struct Explosion {
    pub time_out: u32,
    pub house_id: u8,
    pub is_dirty: bool,
    pub current: u8,
    pub sprite_id: u16,
    /// Index into tables::EXPLOSIONS, or None when free.
    pub commands: Option<u16>,
    pub position: Tile32,
}

/// Which animation table a running animation reads its commands from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimTable {
    UnitMove,
    UnitScript1,
    UnitScript2,
    Map,
    Structure,
}

#[derive(Clone, Copy, Debug)]
pub struct Animation {
    pub tick_next: u32,
    pub tile_layout: u16,
    pub house_id: u8,
    pub current: u8,
    pub icon_group: u8,
    /// (table, row); None when the slot is free.
    pub commands: Option<(AnimTable, u16)>,
    pub tile: Tile32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionType {
    Mentat,
    Target,
    Place,
    Unit,
    Structure,
    Debug,
    Unknown6,
    Intro,
}
