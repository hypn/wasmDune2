//! Map_CreateLandscape: the seed-based terrain generator used for every Dune II map.

use super::Map;
use crate::data::image::{Tiles, ICM_ICONGROUP_LANDSCAPE};
use crate::rng::Rng;
use crate::tables::landscape::{Lst, LANDSCAPE_INFO};
use crate::tile::{is_out_of_map, move_by_random, pack_xy, packed_x, packed_y, Tile32};

const OFFSET_TABLE: [[[u16; 4]; 21]; 2] = [
    [
        [0, 0, 4, 0], [4, 0, 4, 4], [0, 0, 0, 4], [0, 4, 4, 4], [0, 0, 0, 2],
        [0, 2, 0, 4], [0, 0, 2, 0], [2, 0, 4, 0], [4, 0, 4, 2], [4, 2, 4, 4],
        [0, 4, 2, 4], [2, 4, 4, 4], [0, 0, 4, 4], [2, 0, 2, 2], [0, 0, 2, 2],
        [4, 0, 2, 2], [0, 2, 2, 2], [2, 2, 4, 2], [2, 2, 0, 4], [2, 2, 4, 4],
        [2, 2, 2, 4],
    ],
    [
        [0, 0, 4, 0], [4, 0, 4, 4], [0, 0, 0, 4], [0, 4, 4, 4], [0, 0, 0, 2],
        [0, 2, 0, 4], [0, 0, 2, 0], [2, 0, 4, 0], [4, 0, 4, 2], [4, 2, 4, 4],
        [0, 4, 2, 4], [2, 4, 4, 4], [4, 0, 0, 4], [2, 0, 2, 2], [0, 0, 2, 2],
        [4, 0, 2, 2], [0, 2, 2, 2], [2, 2, 4, 2], [2, 2, 0, 4], [2, 2, 4, 4],
        [2, 2, 2, 4],
    ],
];

const AROUND: [i16; 21] = [0, -1, 1, -16, 16, -17, 17, -15, 15, -2, 2, -32, 32, -4, 4, -64, 64, -30, 30, -34, 34];

const SAND: u16 = Lst::NormalSand as u16;
const ROCK: u16 = Lst::EntirelyRock as u16;
const DUNE: u16 = Lst::EntirelyDune as u16;
const MOUNTAIN: u16 = Lst::EntirelyMountain as u16;
const SPICE: u16 = Lst::Spice as u16;
const THICK_SPICE: u16 = Lst::ThickSpice as u16;

fn can_become_spice(id: u16) -> bool {
    LANDSCAPE_INFO.get(id as usize).map(|l| l.can_become_spice).unwrap_or(false)
}

/// Map_AddSpiceOnTile, operating on landscape types (before they become sprite ids).
fn add_spice_on_tile(g: &mut [u16], packed: u16) {
    let p = packed as usize;
    match g[p] {
        SPICE => {
            g[p] = THICK_SPICE;
            add_spice_on_tile(g, packed);
        }
        THICK_SPICE => {
            for j in -1i16..=1 {
                for i in -1i16..=1 {
                    let x = (packed_x(packed) as i16 + i) as u16;
                    let y = (packed_y(packed) as i16 + j) as u16;
                    let packed2 = pack_xy(x, y);
                    if is_out_of_map(packed2) {
                        continue;
                    }
                    let p2 = packed2 as usize;
                    if !can_become_spice(g[p2]) {
                        g[p] = SPICE;
                        continue;
                    }
                    if g[p2] != THICK_SPICE {
                        g[p2] = SPICE;
                    }
                }
            }
        }
        id => {
            if can_become_spice(id) {
                g[p] = SPICE;
            }
        }
    }
}

/// Generate the landscape for `seed`. Returns the ground tile id (sprite) per tile, which is
/// stored in both `map.tiles[..].ground_tile_id` and `map.map_tile_id`.
pub fn create_landscape(map: &mut Map, rng: &mut Rng, tiles: &Tiles, seed: u32) {
    rng.seed_256(seed);
    let mut g = vec![0u16; 4096];

    // Place random data on a 4x4 grid.
    let mut memory = [0u8; 273];
    for m in memory.iter_mut().take(272) {
        *m = rng.random_256() & 0xF;
        if *m > 0xA {
            *m = 0xA;
        }
    }

    let mut i = (rng.random_256() & 0xF) as u16 + 1;
    while i != 0 {
        i -= 1;
        let base = rng.random_256() as i16;
        for a in AROUND {
            let index = (base + a).clamp(0, 272) as usize;
            memory[index] = (memory[index].wrapping_add(rng.random_256() & 0xF)) & 0xF;
        }
    }

    let mut i = (rng.random_256() & 0x3) as u16 + 1;
    while i != 0 {
        i -= 1;
        let base = rng.random_256() as i16;
        for a in AROUND {
            let index = (base + a).clamp(0, 272) as usize;
            memory[index] = rng.random_256() & 0x3;
        }
    }

    for j in 0..16u16 {
        for i in 0..16u16 {
            g[pack_xy(i * 4, j * 4) as usize] = memory[(j * 16 + i) as usize] as u16;
        }
    }

    // Average around the 4x4 grid.
    for j in 0..16u16 {
        for i in 0..16u16 {
            for k in 0..21 {
                let o = OFFSET_TABLE[((i + 1) % 2) as usize][k];
                let packed1 = pack_xy(i * 4 + o[0], j * 4 + o[1]);
                let packed2 = pack_xy(i * 4 + o[2], j * 4 + o[3]);
                let packed = ((packed1 as u32 + packed2 as u32) / 2) as u16;
                if is_out_of_map(packed) {
                    continue;
                }
                let packed1 = pack_xy((i * 4 + o[0]) & 0x3F, j * 4 + o[1]);
                let packed2 = pack_xy((i * 4 + o[2]) & 0x3F, j * 4 + o[3]);
                // Out-of-bounds reads as 0, which reproduces the original maps.
                let sprite2 = if packed2 < 4096 { g[packed2 as usize] } else { 0 };
                g[packed as usize] = (g[packed1 as usize] + sprite2 + 1) / 2;
            }
        }
    }

    // Average each tile with its neighbours.
    let mut current_row = [0u16; 64];
    let mut previous_row;
    for j in 0..64usize {
        previous_row = current_row;
        let t = j * 64;
        current_row.copy_from_slice(&g[t..t + 64]);
        for i in 0..64usize {
            let n = [
                if i == 0 || j == 0 { current_row[i] } else { previous_row[i - 1] },
                if j == 0 { current_row[i] } else { previous_row[i] },
                if i == 63 || j == 0 { current_row[i] } else { previous_row[i + 1] },
                if i == 0 { current_row[i] } else { current_row[i - 1] },
                current_row[i],
                if i == 63 { current_row[i] } else { current_row[i + 1] },
                if i == 0 || j == 63 { current_row[i] } else { g[t + i + 63] },
                if j == 63 { current_row[i] } else { g[t + i + 64] },
                if i == 63 || j == 63 { current_row[i] } else { g[t + i + 65] },
            ];
            let total: u16 = n.iter().sum();
            g[t + i] = total / 9;
        }
    }

    // Filter each tile to determine its final type.
    let mut sprite_id1 = (rng.random_256() & 0xF) as u16;
    sprite_id1 = sprite_id1.clamp(0x8, 0xC);
    let mut sprite_id2 = ((rng.random_256() & 0x3) as u16).wrapping_sub(1);
    if sprite_id2 > sprite_id1 - 3 {
        sprite_id2 = sprite_id1 - 3;
    }
    for v in g.iter_mut() {
        let s = *v;
        *v = if s > sprite_id1 + 4 {
            MOUNTAIN
        } else if s >= sprite_id1 {
            ROCK
        } else if s <= sprite_id2 {
            DUNE
        } else {
            SAND
        };
    }

    // Add some spice.
    let mut i = (rng.random_256() & 0x2F) as u16;
    while i != 0 {
        i -= 1;
        let packed = loop {
            let y = (rng.random_256() & 0x3F) as u16;
            let x = (rng.random_256() & 0x3F) as u16;
            let packed = pack_xy(x, y);
            if can_become_spice(g[packed as usize]) {
                break packed;
            }
        };
        let tile = Tile32::unpack(packed);
        let mut j = (rng.random_256() & 0x1F) as u16;
        while j != 0 {
            j -= 1;
            let packed = loop {
                let d = (rng.random_256() & 0x3F) as u16;
                let packed = move_by_random(rng, tile, d, true).pack();
                if !is_out_of_map(packed) {
                    break packed;
                }
            };
            add_spice_on_tile(&mut g, packed);
        }
    }

    // Make everything smoother and use the right sprite indexes.
    let mut current_row = [0u16; 64];
    for j in 0..64usize {
        let previous_row = current_row;
        let t = j * 64;
        current_row.copy_from_slice(&g[t..t + 64]);
        for i in 0..64usize {
            let current = g[t + i];
            let up = if j == 0 { current } else { previous_row[i] };
            let right = if i == 63 { current } else { current_row[i + 1] };
            let down = if j == 63 { current } else { g[t + i + 64] };
            let left = if i == 0 { current } else { current_row[i - 1] };
            let mut sprite_id = 0u16;
            if up == current { sprite_id |= 1; }
            if right == current { sprite_id |= 2; }
            if down == current { sprite_id |= 4; }
            if left == current { sprite_id |= 8; }
            match current {
                SAND => sprite_id = 0,
                ROCK => {
                    if up == MOUNTAIN { sprite_id |= 1; }
                    if right == MOUNTAIN { sprite_id |= 2; }
                    if down == MOUNTAIN { sprite_id |= 4; }
                    if left == MOUNTAIN { sprite_id |= 8; }
                    sprite_id += 1;
                }
                DUNE => sprite_id += 17,
                MOUNTAIN => sprite_id += 33,
                SPICE => {
                    if up == THICK_SPICE { sprite_id |= 1; }
                    if right == THICK_SPICE { sprite_id |= 2; }
                    if down == THICK_SPICE { sprite_id |= 4; }
                    if left == THICK_SPICE { sprite_id |= 8; }
                    sprite_id += 49;
                }
                THICK_SPICE => sprite_id += 65,
                _ => {}
            }
            g[t + i] = sprite_id;
        }
    }

    // Finalise the tiles with the real sprites.
    let veiled = tiles.veiled_tile();
    for (i, t) in map.tiles.iter_mut().enumerate() {
        *t = Default::default();
        t.ground_tile_id = tiles.group(ICM_ICONGROUP_LANDSCAPE, g[i] as usize);
        t.overlay_tile_id = veiled;
        t.house_id = 0; // HOUSE_HARKONNEN
        map.map_tile_id[i] = t.ground_tile_id;
    }
}
