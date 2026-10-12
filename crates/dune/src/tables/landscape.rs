//! LandscapeInfo table.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Lst {
    NormalSand = 0,
    PartialRock = 1,
    EntirelyDune = 2,
    PartialDune = 3,
    EntirelyRock = 4,
    MostlyRock = 5,
    EntirelyMountain = 6,
    PartialMountain = 7,
    Spice = 8,
    ThickSpice = 9,
    ConcreteSlab = 10,
    Wall = 11,
    Structure = 12,
    DestroyedWall = 13,
    BloomField = 14,
}
pub const LST_MAX: usize = 15;

pub struct LandscapeInfo {
    pub movement_speed: [u8; 6],
    pub let_unit_wobble: bool,
    pub is_valid_for_structure: bool,
    pub is_sand: bool,
    pub is_valid_for_structure2: bool,
    pub can_become_spice: bool,
    pub crater_type: u8,
    pub radar_colour: u16,
    pub sprite_id: u16,
}

const fn li(ms: [u8; 6], wob: bool, vfs: bool, sand: bool, vfs2: bool, spice: bool, crater: u8, radar: u16, sprite: u16) -> LandscapeInfo {
    LandscapeInfo {
        movement_speed: ms,
        let_unit_wobble: wob,
        is_valid_for_structure: vfs,
        is_sand: sand,
        is_valid_for_structure2: vfs2,
        can_become_spice: spice,
        crater_type: crater,
        radar_colour: radar,
        sprite_id: sprite,
    }
}

pub static LANDSCAPE_INFO: [LandscapeInfo; LST_MAX] = [
    li([112, 112, 112, 160, 255, 192], false, false, true, false, true, 1, 88, 37),   // NormalSand
    li([160, 112, 112, 64, 255, 0], true, false, false, false, false, 1, 28, 39),     // PartialRock
    li([112, 160, 160, 160, 255, 192], false, false, true, false, true, 1, 92, 41),   // EntirelyDune
    li([112, 160, 160, 160, 255, 192], false, false, true, false, true, 1, 89, 43),   // PartialDune
    li([112, 160, 160, 112, 255, 0], true, true, false, true, false, 2, 30, 45),      // EntirelyRock
    li([160, 160, 160, 160, 255, 0], true, true, false, true, false, 2, 29, 47),      // MostlyRock
    li([64, 0, 0, 0, 255, 0], true, false, false, false, false, 0, 12, 49),           // EntirelyMountain
    li([64, 0, 0, 0, 255, 0], true, false, false, false, false, 0, 133, 51),          // PartialMountain
    li([112, 160, 160, 160, 255, 192], false, false, true, false, true, 1, 215, 53),  // Spice
    li([112, 160, 160, 160, 255, 192], true, false, true, false, true, 1, 216, 53),   // ThickSpice
    li([255, 255, 255, 255, 255, 0], false, true, false, false, false, 2, 133, 51),   // ConcreteSlab
    li([0, 0, 0, 0, 255, 0], false, false, false, false, false, 0, 65535, 31),        // Wall
    li([0, 0, 0, 0, 255, 0], false, false, false, false, false, 0, 65535, 31),        // Structure
    li([160, 160, 160, 160, 255, 0], true, true, false, true, false, 2, 29, 47),      // DestroyedWall
    li([112, 112, 112, 160, 255, 192], false, false, true, false, true, 1, 50, 57),   // BloomField
];
