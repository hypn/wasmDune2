//! The 64x64 game map.

pub mod landscape;

#[derive(Clone, Copy, Debug, Default)]
pub struct Tile {
    pub ground_tile_id: u16,
    pub overlay_tile_id: u16,
    pub house_id: u8,
    pub is_unveiled: bool,
    pub has_unit: bool,
    pub has_structure: bool,
    pub has_animation: bool,
    pub has_explosion: bool,
    pub index: u8,
}

pub struct Map {
    pub tiles: Vec<Tile>,
    /// g_mapTileID: the original landscape tile id of every tile.
    pub map_tile_id: Vec<u16>,
}

impl Default for Map {
    fn default() -> Self {
        Map { tiles: vec![Tile::default(); 4096], map_tile_id: vec![0; 4096] }
    }
}
