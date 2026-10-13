//! Map logic (map.c, plus the map-dependent parts of tile.c).

use super::*;
use crate::data::image::{ICM_ICONGROUP_FOG_OF_WAR, ICM_ICONGROUP_LANDSCAPE};
use crate::tables::landscape::{Lst, LANDSCAPE_INFO};
use crate::tables::{MAP_DIFF, TILE_DIFF};
use crate::tile::{is_out_of_map, pack_xy, packed_x, packed_y};

/// g_mapInfos: (minX, minY, sizeX, sizeY) per map scale.
pub const MAP_INFOS: [(u16, u16, u16, u16); 3] = [(1, 1, 62, 62), (16, 16, 32, 32), (21, 21, 21, 21)];

const LANDSCAPE_SPRITE_MAP: [u16; 81] = [
    0, 1, 1, 1, 5, 1, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 2, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 6, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
];

pub const LST_NORMAL_SAND: u16 = Lst::NormalSand as u16;
pub const LST_ENTIRELY_DUNE: u16 = Lst::EntirelyDune as u16;
pub const LST_ENTIRELY_ROCK: u16 = Lst::EntirelyRock as u16;
pub const LST_ENTIRELY_MOUNTAIN: u16 = Lst::EntirelyMountain as u16;
pub const LST_PARTIAL_MOUNTAIN: u16 = Lst::PartialMountain as u16;
pub const LST_SPICE: u16 = Lst::Spice as u16;
pub const LST_THICK_SPICE: u16 = Lst::ThickSpice as u16;
pub const LST_CONCRETE_SLAB: u16 = Lst::ConcreteSlab as u16;
pub const LST_WALL: u16 = Lst::Wall as u16;
pub const LST_STRUCTURE: u16 = Lst::Structure as u16;
pub const LST_DESTROYED_WALL: u16 = Lst::DestroyedWall as u16;
pub const LST_BLOOM_FIELD: u16 = Lst::BloomField as u16;

impl Game {
    pub fn map_info(&self) -> (u16, u16, u16, u16) {
        MAP_INFOS[self.scenario.map_scale as usize]
    }

    /// Tile_IsUnveiled
    pub fn tile_is_unveiled(&self, tile_id: u16) -> bool {
        tile_id > self.veiled_tile_id || tile_id < self.veiled_tile_id.wrapping_sub(15)
    }

    /// Map_IsValidPosition
    pub fn map_is_valid_position(&self, position: u16) -> bool {
        if position & 0xC000 != 0 {
            return false;
        }
        let x = packed_x(position);
        let y = packed_y(position);
        let (min_x, min_y, sx, sy) = self.map_info();
        min_x <= x && x < min_x + sx && min_y <= y && y < min_y + sy
    }

    /// Map_IsPositionUnveiled
    pub fn map_is_position_unveiled(&self, position: u16) -> bool {
        let t = &self.map[(position & 0xFFF) as usize];
        t.is_unveiled && self.tile_is_unveiled(t.overlay_tile_id)
    }

    /// Map_IsPositionInViewport
    pub fn map_is_position_in_viewport(&self, position: Tile32) -> (bool, i16, i16) {
        let x = (position.x >> 4) as i16 - ((packed_x(self.viewport_position) << 4) as i16);
        let y = (position.y >> 4) as i16 - ((packed_y(self.viewport_position) << 4) as i16);
        (x >= -16 && x <= 256 && y >= -16 && y <= 176, x, y)
    }

    /// Map_GetLandscapeType
    pub fn map_get_landscape_type(&self, packed: u16) -> u16 {
        let t = &self.map[(packed & 0xFFF) as usize];
        let g = t.ground_tile_id;
        if g == self.built_slab_tile_id {
            return LST_CONCRETE_SLAB;
        }
        if g == self.bloom_tile_id || g == self.bloom_tile_id + 1 {
            return LST_BLOOM_FIELD;
        }
        if g > self.wall_tile_id && g < self.wall_tile_id + 75 {
            return LST_WALL;
        }
        if t.overlay_tile_id == self.wall_tile_id {
            return LST_DESTROYED_WALL;
        }
        if self.structure_get_by_packed_tile(packed).is_some() {
            return LST_STRUCTURE;
        }
        let offset = g as i32 - self.landscape_tile_id as i32;
        if !(0..=80).contains(&offset) {
            return LST_ENTIRELY_ROCK;
        }
        LANDSCAPE_SPRITE_MAP[offset as usize]
    }

    pub fn landscape_info(&self, packed: u16) -> &'static crate::tables::landscape::LandscapeInfo {
        &LANDSCAPE_INFO[self.map_get_landscape_type(packed) as usize]
    }

    /// Map_UpdateWall
    fn map_update_wall(&mut self, packed: u16) -> bool {
        if self.map_get_landscape_type(packed) != LST_WALL {
            return false;
        }
        let p = packed as usize;
        self.map[p].ground_tile_id = self.map_tile_id[p] & 0x1FF;
        if self.map_is_position_unveiled(packed) {
            self.map[p].overlay_tile_id = self.wall_tile_id;
        }
        self.structure_connect_wall(packed, true);
        true
    }

    /// Map_MakeExplosion
    pub fn map_make_explosion(&mut self, type_: u16, position: Tile32, hitpoints: u16, unit_origin_encoded: u16) {
        let mut type_ = type_;
        let reaction_distance: u16 = if type_ == EXPLOSION_DEATH_HAND { 32 } else { 16 };
        let position_packed = position.pack();

        if hitpoints != 0 {
            let mut find = PoolFind::all();
            while let Some(u) = self.unit_find(&mut find) {
                let distance = get_distance(position, self.u(u).o.position) >> 4;
                if distance >= reaction_distance {
                    continue;
                }
                let ut = self.u(u).o.type_;
                if !(ut == UNIT_SANDWORM && type_ == EXPLOSION_SANDWORM_SWALLOW) && ut != UNIT_FRIGATE {
                    self.unit_damage(u, hitpoints >> (distance >> 2), 0);
                }
                if self.u(u).o.house_id == self.player_house_id {
                    continue;
                }
                let Some(us) = self.index_get_unit(unit_origin_encoded) else { continue };
                if us == u {
                    continue;
                }
                if self.house_are_allied(self.unit_get_house_id(u), self.unit_get_house_id(us)) {
                    continue;
                }
                if let Some(t) = self.unit_get_team(u) {
                    if self.teams[t as usize].action == TEAM_ACTION_STAGING {
                        self.unit_remove_from_team(u);
                        self.unit_set_action(u, ACTION_HUNT);
                        continue;
                    }
                    let Some(target) = self.index_get_unit(self.teams[t as usize].target) else { continue };
                    if self.ui(target).bullet_type == UNIT_INVALID {
                        self.teams[t as usize].target = unit_origin_encoded;
                    }
                    continue;
                }
                if self.u(u).o.type_ == UNIT_HARVESTER {
                    let uis_mt = self.ui(us).movement_type;
                    if uis_mt == MOVEMENT_FOOT && self.u(u).target_move == 0 {
                        if self.u(u).action_id != ACTION_MOVE {
                            self.unit_set_action(u, ACTION_MOVE);
                        }
                        self.um(u).target_move = unit_origin_encoded;
                        continue;
                    }
                }
                if self.ui(u).bullet_type == UNIT_INVALID {
                    continue;
                }
                if self.u(u).action_id == ACTION_GUARD && self.u(u).o.flags.by_scenario {
                    self.unit_set_action(u, ACTION_HUNT);
                }
                if self.u(u).target_attack != 0 && self.u(u).action_id != ACTION_HUNT {
                    continue;
                }
                if self.index_get_unit(self.u(u).target_attack).is_some() {
                    let packed = self.u(u).o.position.pack();
                    if get_distance_packed(self.index_get_packed_tile(self.u(u).target_attack), packed) <= self.ui(u).fire_distance {
                        continue;
                    }
                }
                self.unit_set_target(u, unit_origin_encoded);
            }
        }

        if hitpoints != 0 {
            if let Some(s) = self.structure_get_by_packed_tile(position_packed) {
                if type_ == EXPLOSION_IMPACT_LARGE && self.si(s).o.hitpoints / 2 > self.s(s).o.hitpoints {
                    type_ = EXPLOSION_SMOKE_PLUME;
                }
                let h = self.s(s).o.house_id;
                self.structure_house_under_attack(h);
                self.structure_damage(s, hitpoints, 0);
            }
        }

        if self.map_get_landscape_type(position_packed) == LST_WALL && hitpoints != 0 {
            let wall_hp = self.structure_info[STRUCTURE_WALL as usize].o.hitpoints;
            if wall_hp <= hitpoints || (self.rng.random_256() as u32) <= (hitpoints as u32 * 256 / wall_hp as u32) {
                self.map_update_wall(position_packed);
            }
        }

        self.explosion_start(type_, position);
    }

    /// Map_DeviateArea
    pub fn map_deviate_area(&mut self, type_: u16, position: Tile32, radius: u16, house_id: u8) {
        self.explosion_start(type_, position);
        let mut find = PoolFind::all();
        while let Some(u) = self.unit_find(&mut find) {
            if get_distance(position, self.u(u).o.position) / 16 >= radius {
                continue;
            }
            self.unit_deviate(u, 0, house_id);
        }
    }

    /// Map_Bloom_ExplodeSpice
    pub fn map_bloom_explode_spice(&mut self, packed: u16, house_id: u8) {
        if self.validate_strict_if_zero == 0 {
            if let Some(u) = self.unit_get_by_packed_tile(packed) {
                self.unit_remove(u);
            }
            self.map[packed as usize].ground_tile_id = self.map_tile_id[packed as usize] & 0x1FF;
            self.map_make_explosion(EXPLOSION_SPICE_BLOOM_TREMOR, Tile32::unpack(packed), 0, 0);
        }
        if house_id == self.player_house_id {
            self.feedback(36);
        }
        self.map_fill_circle_with_spice(packed, 5);
    }

    /// Map_FillCircleWithSpice
    pub fn map_fill_circle_with_spice(&mut self, packed: u16, radius: u16) {
        if radius == 0 {
            return;
        }
        let x = packed_x(packed) as i32;
        let y = packed_y(packed) as i32;
        let r = radius as i32;
        for i in -r..=r {
            for j in -r..=r {
                let cur = pack_xy((x + j) as u16, (y + i) as u16);
                let distance = get_distance_packed(packed, cur);
                if distance > radius {
                    continue;
                }
                if distance == radius && (self.rng.random_256() & 1) == 0 {
                    continue;
                }
                if self.map_get_landscape_type(cur) == LST_SPICE {
                    continue;
                }
                self.map_change_spice_amount(cur, 1);
            }
        }
        self.map_change_spice_amount(packed, 1);
    }

    /// Map_FixupSpiceEdges
    fn map_fixup_spice_edges(&mut self, packed: u16) {
        let packed = packed & 0xFFF;
        let type_ = self.map_get_landscape_type(packed);
        let mut sprite_id: u16 = 0;
        if type_ == LST_SPICE || type_ == LST_THICK_SPICE {
            for i in 0..4 {
                let cur = packed.wrapping_add(MAP_DIFF[i] as u16);
                if is_out_of_map(cur) {
                    sprite_id |= 1 << i;
                    continue;
                }
                let cur_type = self.map_get_landscape_type(cur);
                if type_ == LST_SPICE {
                    if cur_type == LST_SPICE || cur_type == LST_THICK_SPICE {
                        sprite_id |= 1 << i;
                    }
                    continue;
                }
                if cur_type == LST_THICK_SPICE {
                    sprite_id |= 1 << i;
                }
            }
            sprite_id += if type_ == LST_SPICE { 49 } else { 65 };
            let sprite_id = self.icon_map[self.icon_map[ICM_ICONGROUP_LANDSCAPE] as usize + sprite_id as usize] & 0x1FF;
            self.map_tile_id[packed as usize] = 0x8000 | sprite_id;
            self.map[packed as usize].ground_tile_id = sprite_id;
        }
    }

    /// Map_ChangeSpiceAmount
    pub fn map_change_spice_amount(&mut self, packed: u16, dir: i16) {
        if dir == 0 {
            return;
        }
        let packed = packed & 0xFFF;
        let mut type_ = self.map_get_landscape_type(packed);
        if type_ == LST_THICK_SPICE && dir > 0 {
            return;
        }
        if type_ != LST_SPICE && type_ != LST_THICK_SPICE && dir < 0 {
            return;
        }
        if type_ != LST_NORMAL_SAND && type_ != LST_ENTIRELY_DUNE && type_ != LST_SPICE && dir > 0 {
            return;
        }
        if dir > 0 {
            type_ = if type_ == LST_SPICE { LST_THICK_SPICE } else { LST_SPICE };
        } else {
            type_ = if type_ == LST_THICK_SPICE { LST_SPICE } else { LST_NORMAL_SAND };
        }
        let mut sprite_id = 0u16;
        if type_ == LST_SPICE {
            sprite_id = 49;
        }
        if type_ == LST_THICK_SPICE {
            sprite_id = 65;
        }
        let sprite_id = self.icon_map[self.icon_map[ICM_ICONGROUP_LANDSCAPE] as usize + sprite_id as usize] & 0x1FF;
        self.map_tile_id[packed as usize] = 0x8000 | sprite_id;
        self.map[packed as usize].ground_tile_id = sprite_id;
        self.map_fixup_spice_edges(packed);
        self.map_fixup_spice_edges(packed.wrapping_add(1));
        self.map_fixup_spice_edges(packed.wrapping_sub(1));
        self.map_fixup_spice_edges(packed.wrapping_sub(64));
        self.map_fixup_spice_edges(packed.wrapping_add(64));
    }

    /// Map_SetViewportPosition
    pub fn map_set_viewport_position(&mut self, packed: u16) {
        let (min_x, min_y, sx, sy) = self.map_info();
        let x = packed_x(packed) as i16 - 7;
        let y = packed_y(packed) as i16 - 5;
        let x = (min_x as i16).max(((min_x + sx) as i16 - 15).min(x));
        let y = (min_y as i16).max(((min_y + sy) as i16 - 10).min(y));
        self.viewport_position = pack_xy(x as u16, y as u16);
    }

    /// Map_Bloom_ExplodeSpecial
    pub fn map_bloom_explode_special(&mut self, packed: u16, house_id: u8) {
        self.map[packed as usize].ground_tile_id = self.landscape_tile_id;
        self.map_tile_id[packed as usize] = 0x8000 | self.landscape_tile_id;
        let mut enemy_house_id = house_id;
        let mut find = PoolFind::all();
        while let Some(u) = self.unit_find(&mut find) {
            if self.u(u).o.house_id == house_id {
                continue;
            }
            enemy_house_id = self.u(u).o.house_id;
            break;
        }
        match self.rng.random_256() & 0x3 {
            0 => {
                let c = self.rng.lcg_range(150, 400);
                let h = self.hm(house_id);
                h.credits = h.credits.wrapping_add(c);
            }
            1 => {
                let position = self.tile_move_by_random(Tile32::unpack(packed), 16, true);
                let o = self.rng.random_256() as i8;
                self.unit_create(INDEX_INVALID, UNIT_TRIKE, house_id, position, o);
            }
            2 => {
                let position = self.tile_move_by_random(Tile32::unpack(packed), 16, true);
                let o = self.rng.random_256() as i8;
                if let Some(u) = self.unit_create(INDEX_INVALID, UNIT_TRIKE, enemy_house_id, position, o) {
                    self.unit_set_action(u, ACTION_HUNT);
                }
            }
            _ => {
                let position = self.tile_move_by_random(Tile32::unpack(packed), 16, true);
                let o = self.rng.random_256() as i8;
                if let Some(u) = self.unit_create(INDEX_INVALID, UNIT_INFANTRY, enemy_house_id, position, o) {
                    self.unit_set_action(u, ACTION_HUNT);
                }
            }
        }
    }

    /// Map_FindLocationTile
    pub fn map_find_location_tile(&mut self, location_id: u16, house_id: u8) -> u16 {
        const MAP_BASE: [i16; 3] = [1, -2, -2];
        let (min_x, min_y, sx, sy) = self.map_info();
        let map_offset = MAP_BASE[self.scenario.map_scale as usize];
        let mut house_id = house_id;
        let mut ret: u16 = 0;

        if location_id == 6 {
            let mut find = PoolFind::all();
            while let Some(s) = self.structure_find(&mut find) {
                let t = self.s(s).o.type_;
                if t == STRUCTURE_SLAB_1X1 || t == STRUCTURE_SLAB_2X2 || t == STRUCTURE_WALL {
                    continue;
                }
                if self.s(s).o.house_id == house_id {
                    continue;
                }
                house_id = self.s(s).o.house_id;
                break;
            }
        }

        while ret == 0 {
            match location_id {
                0 => {
                    let x = min_x + self.rng.lcg_range(0, sx - 2);
                    ret = pack_xy(x, (min_y as i16 + map_offset) as u16);
                }
                1 => {
                    let y = min_y + self.rng.lcg_range(0, sy - 2);
                    ret = pack_xy((min_x as i16 + sx as i16 - map_offset) as u16, y);
                }
                2 => {
                    let x = min_x + self.rng.lcg_range(0, sx - 2);
                    ret = pack_xy(x, (min_y as i16 + sy as i16 - map_offset) as u16);
                }
                3 => {
                    let y = min_y + self.rng.lcg_range(0, sy - 2);
                    ret = pack_xy((min_x as i16 + map_offset) as u16, y);
                }
                4 => {
                    let x = min_x + self.rng.lcg_range(0, sx);
                    let y = min_y + self.rng.lcg_range(0, sy);
                    ret = pack_xy(x, y);
                    if house_id == self.player_house_id && !self.map_is_valid_position(ret) {
                        ret = 0;
                    }
                }
                5 => {
                    let x = packed_x(self.minimap_position) + self.rng.lcg_range(0, 14);
                    let y = packed_y(self.minimap_position) + self.rng.lcg_range(0, 9);
                    ret = pack_xy(x, y);
                    if house_id == self.player_house_id && !self.map_is_valid_position(ret) {
                        ret = 0;
                    }
                }
                6 | 7 => {
                    let mut find = PoolFind::new(house_id, 0xFFFF);
                    if let Some(s) = self.structure_find(&mut find) {
                        let pos = self.s(s).o.position;
                        ret = self.tile_move_by_random(pos, 120, true).pack();
                    } else {
                        let mut find = PoolFind::new(house_id, 0xFFFF);
                        if let Some(u) = self.unit_find(&mut find) {
                            let pos = self.u(u).o.position;
                            ret = self.tile_move_by_random(pos, 120, true).pack();
                        } else {
                            let x = min_x + self.rng.lcg_range(0, sx);
                            let y = min_y + self.rng.lcg_range(0, sy);
                            ret = pack_xy(x, y);
                        }
                    }
                    if house_id == self.player_house_id && !self.map_is_valid_position(ret) {
                        ret = 0;
                    }
                }
                _ => return 0,
            }
            ret &= 0xFFF;
            if ret != 0 && self.object_get_by_packed_tile(ret).is_some() {
                ret = 0;
            }
        }
        ret
    }

    /// Map_UpdateAround: for each tile touched by an object of `radius` (pixels) at `position`,
    /// call function 2 (Unit_RemoveFromTile) or 3 (Unit_AddToTile). Functions 0/1 only
    /// invalidate the display, which we redraw fully every frame.
    pub fn map_update_around(&mut self, radius: u16, position: Tile32, unit: Option<u16>, function: u8) {
        const TILE_OFFSETS: [u16; 18] = [
            0x0080, 0x0088, 0x0090, 0x0098, 0x00A0, 0x00A8, 0x00B0, 0x00B8, 0x00C0, 0x00C8, 0x00D0, 0x00D8, 0x00E0, 0x00E8, 0x00F0,
            0x00F8, 0x0100, 0x0180,
        ];
        if function < 2 {
            return;
        }
        if radius == 0 || (position.x == 0 && position.y == 0) {
            return;
        }
        let radius = radius - 1;
        let apply = |g: &mut Game, cur: u16| {
            let Some(u) = unit else { return };
            match function {
                2 => g.unit_remove_from_tile(u, cur),
                3 => g.unit_add_to_tile(u, cur),
                _ => {}
            }
        };
        if radius >= 32 {
            let x = position.pos_x() as i32;
            let y = position.pos_y() as i32;
            for i in -2..=2 {
                for j in -2..=2 {
                    if x + i < 0 || x + i >= 64 || y + j < 0 || y + j >= 64 {
                        continue;
                    }
                    apply(self, pack_xy((x + i) as u16, (y + j) as u16));
                }
            }
            return;
        }
        let radius = radius.max(15);
        let mut position = position;
        position.x = position.x.wrapping_sub(TILE_OFFSETS[(radius - 15) as usize]);
        position.y = position.y.wrapping_sub(TILE_OFFSETS[(radius - 15) as usize]);
        let mut diff = Tile32::default();
        let mut last_packed = 0u16;
        let mut i = 0usize;
        loop {
            let cur_tile = add_tile_diff(position, diff);
            if cur_tile.is_valid() {
                let cur = cur_tile.pack();
                if cur != last_packed {
                    apply(self, cur);
                    last_packed = cur;
                }
            }
            if i == 8 {
                break;
            }
            diff = TILE_DIFF[radius as usize + 1][i];
            i += 1;
            if diff.x == 0 && diff.y == 0 {
                break;
            }
        }
    }

    /// Map_SearchSpice
    pub fn map_search_spice(&self, packed: u16, radius: u16) -> u16 {
        self.map_search_spice_where(packed, radius, false)
    }

    /// Map_SearchSpice, optionally only over tiles the player has seen (for qol.rs).
    pub fn map_search_spice_where(&self, packed: u16, radius: u16, seen_only: bool) -> u16 {
        // ENHANCEMENT -- the original only accepts spice within `radius`, but searches a square;
        // when the only spice is in the corners it returns the start tile and harvesters get stuck.
        let mut radius1 = if self.config.dune2_enhanced { u16::MAX } else { radius + 1 };
        let mut radius2 = radius + 1;
        let mut packed1 = packed;
        let mut packed2 = packed;
        let mut found = false;
        let (min_x, min_y, sx, sy) = self.map_info();
        let xmin = (packed_x(packed) as i32 - radius as i32).max(min_x as i32) as u16;
        let xmax = (packed_x(packed) + radius).min(min_x + sx - 1);
        let ymin = (packed_y(packed) as i32 - radius as i32).max(min_y as i32) as u16;
        let ymax = (packed_y(packed) + radius).min(min_y + sy - 1);
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                let cur = pack_xy(x, y);
                if !self.map_is_valid_position(cur) {
                    continue;
                }
                if self.map[cur as usize].has_structure {
                    continue;
                }
                if seen_only && !self.map_is_position_unveiled(cur) {
                    continue;
                }
                if self.unit_get_by_packed_tile(cur).is_some() {
                    continue;
                }
                let type_ = self.map_get_landscape_type(cur);
                let distance = get_distance_packed(cur, packed);
                if type_ == LST_THICK_SPICE && distance < 4 {
                    found = true;
                    if distance <= radius2 {
                        radius2 = distance;
                        packed2 = cur;
                    }
                }
                if type_ == LST_SPICE {
                    found = true;
                    if distance <= radius1 {
                        radius1 = distance;
                        packed1 = cur;
                    }
                }
            }
        }
        if !found {
            return 0;
        }
        if radius2 <= radius {
            packed2
        } else {
            packed1
        }
    }

    /// Map_UnveilTile_Neighbour
    fn map_unveil_tile_neighbour(&mut self, packed: u16) {
        if is_out_of_map(packed) {
            return;
        }
        let mut tile_id: u16 = 15;
        if self.map[packed as usize].is_unveiled {
            if self.validate_strict_if_zero == 0 && self.tile_is_unveiled(self.map[packed as usize].overlay_tile_id) {
                return;
            }
            tile_id = 0;
            for i in 0..4 {
                let n = packed.wrapping_add(MAP_DIFF[i] as u16);
                if is_out_of_map(n) || !self.map[n as usize].is_unveiled {
                    tile_id |= 1 << i;
                }
            }
        }
        if tile_id != 0 {
            if tile_id != 15 {
                if let Some(u) = self.unit_get_by_packed_tile(packed) {
                    let p = self.player_house_id;
                    self.unit_house_unit_count_add(u, p);
                }
            }
            tile_id = self.icon_map[self.icon_map[ICM_ICONGROUP_FOG_OF_WAR] as usize + tile_id as usize];
        }
        self.map[packed as usize].overlay_tile_id = tile_id;
    }

    /// Map_UnveilTile
    pub fn map_unveil_tile(&mut self, packed: u16, house_id: u8) -> bool {
        if house_id != self.player_house_id {
            return false;
        }
        if is_out_of_map(packed) {
            return false;
        }
        let t = &self.map[packed as usize];
        if t.is_unveiled && self.tile_is_unveiled(t.overlay_tile_id) {
            return false;
        }
        self.map[packed as usize].is_unveiled = true;
        if let Some(u) = self.unit_get_by_packed_tile(packed) {
            self.unit_house_unit_count_add(u, house_id);
        }
        if let Some(s) = self.structure_get_by_packed_tile(packed) {
            let sm = self.sm(s);
            sm.o.seen_by_houses |= 1 << house_id;
            if house_id == HOUSE_ATREIDES {
                sm.o.seen_by_houses |= 1 << HOUSE_FREMEN;
            }
        }
        self.map_unveil_tile_neighbour(packed);
        self.map_unveil_tile_neighbour(packed.wrapping_add(1));
        self.map_unveil_tile_neighbour(packed.wrapping_sub(1));
        self.map_unveil_tile_neighbour(packed.wrapping_sub(64));
        self.map_unveil_tile_neighbour(packed.wrapping_add(64));
        true
    }

    /// Tile_RemoveFogInRadius
    pub fn tile_remove_fog_in_radius(&mut self, tile: Tile32, radius: u16) {
        let packed = tile.pack();
        if !self.map_is_valid_position(packed) {
            return;
        }
        let x = packed_x(packed) as i32;
        let y = packed_y(packed) as i32;
        let tile = Tile32::make_xy(x as u16, y as u16);
        let r = radius as i32;
        for i in -r..=r {
            for j in -r..=r {
                if x + i < 0 || x + i >= 64 || y + j < 0 || y + j >= 64 {
                    continue;
                }
                let p = pack_xy((x + i) as u16, (y + j) as u16);
                let t = Tile32::make_xy((x + i) as u16, (y + j) as u16);
                if get_distance_rounded_up(tile, t) > radius {
                    continue;
                }
                let ph = self.player_house_id;
                self.map_unveil_tile(p, ph);
            }
        }
    }

    /// Tile_GetTileInDirectionOf
    pub fn tile_get_tile_in_direction_of(&mut self, packed_from: u16, packed_to: u16) -> u16 {
        if packed_from == 0 || packed_to == 0 {
            return 0;
        }
        let distance = get_distance_packed(packed_from, packed_to) as i16;
        let direction = get_direction_packed(packed_to, packed_from);
        if distance <= 10 {
            return 0;
        }
        loop {
            let mut dir: i16 = 31 + (self.rng.random_256() & 0x3F) as i16;
            if self.rng.random_256() & 1 != 0 {
                dir = -dir;
            }
            let position = Tile32::unpack(packed_to);
            let position = move_by_direction(position, direction as i16 + dir, (distance.min(20) as u16) << 8);
            let packed = position.pack();
            if self.map_is_valid_position(packed) {
                return packed;
            }
        }
    }

    /// Map_SetSelection (game-side parts: selecting units / structures).
    pub fn map_set_selection(&mut self, packed: u16) {
        if self.selection_type == SelectionType::Target {
            return;
        }
        if self.selection_type == SelectionType::Place {
            self.selection_state = self.structure_is_valid_build_location(packed, self.structure_active_type as u8);
            self.selection_position = packed;
            return;
        }
        let mut packed = packed;
        if packed != 0xFFFF && self.map[(packed & 0xFFF) as usize].overlay_tile_id != self.veiled_tile_id {
            if let Some(s) = self.structure_get_by_packed_tile(packed) {
                let si = *self.si(s);
                if self.s(s).o.house_id == self.player_house_id && self.selection_type != SelectionType::Mentat {
                    self.emit(Event::Hint(si.o.hint_string_id, si.o.sprite_id));
                }
                packed = self.s(s).o.position.pack();
                self.map_set_selection_size(si.layout);
            } else {
                self.map_set_selection_size(0);
            }
            if self.selection_type != SelectionType::Target {
                match self.unit_get_by_packed_tile(packed) {
                    Some(u) => {
                        if self.u(u).o.type_ != UNIT_CARRYALL {
                            self.unit_select(Some(u));
                        }
                    }
                    None => {
                        if self.unit_selected.is_some() {
                            self.unit_select(None);
                        }
                    }
                }
            }
            self.selection_position = packed;
            return;
        }
        self.map_set_selection_size(0);
        self.selection_position = packed;
    }

    /// Map_SetSelectionSize
    pub fn map_set_selection_size(&mut self, layout: u16) -> u16 {
        let old = self.selection_layout;
        if layout & 0x80 != 0 {
            return old;
        }
        self.selection_layout = layout;
        let (w, h) = crate::tables::STRUCTURE_LAYOUT_SIZE[layout as usize];
        self.selection_width = w;
        self.selection_height = h;
        old
    }
}
