//! Loaders for the original Westwood data formats.

pub mod format80;
pub mod image;
pub mod pak;
pub mod strings;
pub mod wsa;

use image::{Palette, Sprite, Tiles};
use pak::Vfs;

/// PAK files the game may use, in load order. Missing ones are skipped (shareware data).
pub const KNOWN_PAKS: &[&str] = &[
    "DUNE.PAK", "ENGLISH.PAK", "SCENARIO.PAK", "MENTAT.PAK", "VOC.PAK", "MERC.PAK", "FINALE.PAK",
    "INTRO.PAK", "INTROVOC.PAK", "SOUND.PAK", "ATRE.PAK", "HARK.PAK", "ORDOS.PAK",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataVariant {
    /// Full v1.07 game.
    Full,
    /// The November 1992 demo: no Harkonnen/Ordos PAKs, no CHOAM, etc.
    Shareware,
}

/// Every asset the game needs, decoded once at start-up.
pub struct Assets {
    pub vfs: Vfs,
    pub variant: DataVariant,
    pub palette: Palette,
    pub tiles: Tiles,
    /// g_sprites, in the original numbering (missing files leave `None` holes).
    pub sprites: Vec<Option<Sprite>>,
}

impl Assets {
    pub fn load(vfs: Vfs) -> Result<Assets, String> {
        let need = |n: &str| vfs.get(n).ok_or_else(|| format!("missing {n}"));
        let variant = if vfs.has_pak("HARK.PAK") && vfs.has_pak("ORDOS.PAK") && vfs.exists("CHOAM.CPS") {
            DataVariant::Full
        } else {
            DataVariant::Shareware
        };
        let palette = Palette::from_bytes(need("IBM.PAL")?);
        let tiles = Tiles::load(need("ICON.ICN")?, need("ICON.MAP")?)?;

        // Same order and expected counts as Sprites_Init, so sprite ids match the original.
        let mut sprites = Vec::new();
        let mut add = |name: &str, alt: Option<&str>, expected: usize| {
            let file = vfs.get(name).or_else(|| alt.and_then(|a| vfs.get(a)));
            match file {
                Some(d) => {
                    let mut s = image::load_shp(d);
                    if expected == 99 && s.len() == 103 {
                        // Dune II v1.0 SHAPES.SHP carries the 4 BTTN sprites at 94..98: move them
                        // to slots 7..10 so sprite ids match v1.07.
                        let bttn: Vec<_> = s.drain(94..98).collect();
                        for (i, b) in bttn.into_iter().enumerate() {
                            sprites[7 + i] = b;
                        }
                    }
                    s.resize(expected, None);
                    sprites.extend(s);
                }
                None => sprites.extend(std::iter::repeat_n(None, expected)),
            }
        };
        add("MOUSE.SHP", None, 7);
        add("BTTN.ENG", None, 5);
        add("SHAPES.SHP", None, 99);
        add("UNITS2.SHP", None, 40);
        add("UNITS1.SHP", None, 87);
        add("UNITS.SHP", None, 117);
        add("CHOAM.ENG", Some("CHOAMSHP.SHP"), 18);
        add("MENTAT.ENG", Some("MENTAT.SHP"), 14);
        add("MENSHPH.SHP", None, 15);
        add("MENSHPA.SHP", None, 15);
        add("MENSHPO.SHP", None, 15);
        add("MENSHPM.SHP", None, 15);
        add("MENSHPM.SHP", None, 15);
        add("MENSHPM.SHP", None, 15);
        add("PIECES.SHP", None, 28);
        add("ARROWS.SHP", None, 9);
        for i in 1..=11 {
            add(&format!("CREDIT{i}.SHP"), None, 1);
        }

        Ok(Assets { vfs, variant, palette, tiles, sprites })
    }
}
