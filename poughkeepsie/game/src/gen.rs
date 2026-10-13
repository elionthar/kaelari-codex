//! Seeded per-chunk content: loot spots inside buildings and zombie spawns outdoors.

use pkmap::{Tile, CHUNK, CHUNK_TILES};
use rand::Rng;

use crate::seed::chunk_rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LootKind {
    Food,
    Water,
    Medkit,
    Ammo,
    Scrap,
}

impl LootKind {
    pub const ALL: [LootKind; 5] = [LootKind::Food, LootKind::Water, LootKind::Medkit, LootKind::Ammo, LootKind::Scrap];

    pub fn name(self) -> &'static str {
        match self {
            LootKind::Food => "Food",
            LootKind::Water => "Water",
            LootKind::Medkit => "Medkit",
            LootKind::Ammo => "Ammo",
            LootKind::Scrap => "Scrap",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loot {
    /// World tile coordinates.
    pub x: i32,
    pub y: i32,
    pub kind: LootKind,
}

/// Chance that a given building floor tile holds a loot spot.
const LOOT_PER_FLOOR_TILE: f64 = 1.0 / 90.0;

pub fn chunk_loot(world_seed: u64, cx: i32, cy: i32, tiles: &[u8; CHUNK_TILES]) -> Vec<Loot> {
    let mut rng = chunk_rng(world_seed, "loot", cx, cy);
    let mut out = Vec::new();
    for (i, &t) in tiles.iter().enumerate() {
        // Roll for every tile (not just floors) so the stream stays aligned per tile.
        let roll = rng.gen_bool(LOOT_PER_FLOOR_TILE);
        let kind_roll: u32 = rng.gen_range(0..100);
        if !roll || t != Tile::Floor as u8 {
            continue;
        }
        let kind = match kind_roll {
            0..=29 => LootKind::Food,
            30..=54 => LootKind::Water,
            55..=69 => LootKind::Medkit,
            70..=79 => LootKind::Ammo,
            _ => LootKind::Scrap,
        };
        out.push(Loot { x: cx * CHUNK as i32 + (i % CHUNK) as i32, y: cy * CHUNK as i32 + (i / CHUNK) as i32, kind });
    }
    out
}

/// Zombie spawn points (tile centers, world coords) for a chunk.
pub fn chunk_zombies(world_seed: u64, cx: i32, cy: i32, tiles: &[u8; CHUNK_TILES]) -> Vec<(f32, f32)> {
    let mut rng = chunk_rng(world_seed, "zombies", cx, cy);
    let count = match rng.gen_range(0..10) {
        0..=4 => 0,
        5..=7 => 1,
        8 => 2,
        _ => 3,
    };
    let mut out = Vec::new();
    for _ in 0..count {
        // A few tries to land on open ground outside buildings.
        for _ in 0..12 {
            let i = rng.gen_range(0..CHUNK_TILES);
            let t = Tile::from_u8(tiles[i]);
            if matches!(t, Tile::Road | Tile::Path | Tile::Ground | Tile::Grass | Tile::Parking) {
                let x = (cx * CHUNK as i32 + (i % CHUNK) as i32) as f32 + 0.5;
                let y = (cy * CHUNK as i32 + (i / CHUNK) as i32) as f32 + 0.5;
                out.push((x, y));
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loot_only_on_floors_and_deterministic() {
        let mut tiles = [Tile::Road as u8; CHUNK_TILES];
        for t in tiles.iter_mut().take(CHUNK_TILES / 2) {
            *t = Tile::Floor as u8;
        }
        let a = chunk_loot(42, 1, 2, &tiles);
        assert_eq!(a, chunk_loot(42, 1, 2, &tiles));
        assert!(!a.is_empty());
        assert_ne!(a, chunk_loot(43, 1, 2, &tiles));
        for l in &a {
            let i = (l.y - 2 * CHUNK as i32) as usize * CHUNK + (l.x - CHUNK as i32) as usize;
            assert_eq!(tiles[i], Tile::Floor as u8);
        }
        let z = chunk_zombies(42, 1, 2, &tiles);
        assert_eq!(z, chunk_zombies(42, 1, 2, &tiles));
    }
}
