//! Types of the static game tables (layout mirrors OpenDUNE's).

#[derive(Clone, Copy, Debug)]
pub struct ObjectInfoFlags {
    pub has_shadow: bool,
    pub factory: bool,
    pub not_on_concrete: bool,
    pub busy_state_is_incoming: bool,
    pub blur_tile: bool,
    pub has_turret: bool,
    pub conquerable: bool,
    pub can_be_picked_up: bool,
    pub no_message_on_death: bool,
    pub tab_selectable: bool,
    pub script_no_slowdown: bool,
    pub target_air: bool,
    pub priority: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectInfo {
    pub string_id_abbrev: u16,
    pub name: &'static str,
    pub string_id_full: u16,
    pub wsa: &'static str,
    pub flags: ObjectInfoFlags,
    pub spawn_chance: u16,
    pub hitpoints: u16,
    pub fog_uncover_radius: u16,
    pub sprite_id: u16,
    pub build_credits: u16,
    pub build_time: u16,
    pub available_campaign: u16,
    pub structures_required: u32,
    pub sort_priority: u8,
    pub upgrade_level_required: u8,
    pub actions_player: [u16; 4],
    pub available: i8,
    pub hint_string_id: u16,
    pub priority_build: u16,
    pub priority_target: u16,
    pub available_house: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct UnitInfoFlags {
    pub is_bullet: bool,
    pub explode_on_death: bool,
    pub sonic_protection: bool,
    pub can_wobble: bool,
    pub is_tracked: bool,
    pub is_ground_unit: bool,
    pub must_stay_in_map: bool,
    pub fires_twice: bool,
    pub impact_on_sand: bool,
    pub is_not_deviatable: bool,
    pub has_animation_set: bool,
    pub not_accurate: bool,
    pub is_normal_unit: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct UnitInfo {
    pub o: ObjectInfo,
    pub index_start: u16,
    pub index_end: u16,
    pub flags: UnitInfoFlags,
    pub dimension: u16,
    pub movement_type: u16,
    pub animation_speed: u16,
    pub moving_speed_factor: u16,
    pub turning_speed: u8,
    pub ground_sprite_id: u16,
    pub turret_sprite_id: u16,
    pub action_ai: u16,
    pub display_mode: u16,
    pub destroyed_sprite_id: u16,
    pub fire_delay: u16,
    pub fire_distance: u16,
    pub damage: u16,
    pub explosion_type: u16,
    pub bullet_type: u8,
    pub bullet_sound: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct StructureInfo {
    pub o: ObjectInfo,
    pub enter_filter: u32,
    pub credits_storage: u16,
    pub power_usage: i16,
    pub layout: u16,
    pub icon_group: u16,
    pub animation_index: [u8; 3],
    pub buildable_units: [u8; 8],
    pub upgrade_campaign: [u16; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct HouseInfo {
    pub name: &'static str,
    pub toughness: u16,
    pub degrading_chance: u16,
    pub degrading_amount: u16,
    pub minimap_color: u16,
    pub special_count_down: u16,
    pub starport_delivery_time: u16,
    pub prefix_char: u8,
    pub special_weapon: u16,
    pub music_win: u16,
    pub music_lose: u16,
    pub music_briefing: u16,
    pub voice_filename: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct ActionInfo {
    pub string_id: u16,
    pub name: &'static str,
    pub switch_type: u16,
    pub selection_type: u16,
    pub sound_id: u16,
}

/// (command, parameter)
#[derive(Clone, Copy, Debug)]
pub struct AnimationCmd(pub u8, pub u16);

/// (command, parameter)
#[derive(Clone, Copy, Debug)]
pub struct ExplosionCmd(pub u8, pub u16);
