//! Chunk streaming: decode chunks near the player, build one tiny texture per chunk
//! (1 pixel per tile) and drop chunks that fall out of range.

use std::collections::{HashMap, HashSet};

use macroquad::prelude::*;
use pkmap::{PkMap, Tile, CHUNK, CHUNK_TILES};

use crate::gen::{chunk_loot, chunk_zombies, Loot};

pub struct Chunk {
    pub tiles: [u8; CHUNK_TILES],
    pub loot: Vec<Loot>,
    tex: Option<Texture2D>,
}

pub struct World {
    pub map: PkMap,
    pub seed: u64,
    chunks: HashMap<(i32, i32), Chunk>,
    /// Loot tiles already picked up this run (so reloaded chunks don't refill).
    taken: HashSet<(i32, i32)>,
    /// Chunks whose zombies have been spawned this run.
    populated: HashSet<(i32, i32)>,
}

pub fn chunk_of(x: i32, y: i32) -> (i32, i32) {
    (x.div_euclid(CHUNK as i32), y.div_euclid(CHUNK as i32))
}

impl World {
    pub fn new(map: PkMap, seed: u64) -> World {
        World { map, seed, chunks: HashMap::new(), taken: HashSet::new(), populated: HashSet::new() }
    }

    fn chunk(&mut self, cx: i32, cy: i32) -> &mut Chunk {
        let (map, seed, taken) = (&self.map, self.seed, &self.taken);
        self.chunks.entry((cx, cy)).or_insert_with(|| {
            let mut tiles = [0u8; CHUNK_TILES];
            map.decode_chunk(cx, cy, &mut tiles);
            let mut loot = chunk_loot(seed, cx, cy, &tiles);
            loot.retain(|l| !taken.contains(&(l.x, l.y)));
            Chunk { tiles, loot, tex: None }
        })
    }

    pub fn tile(&mut self, x: i32, y: i32) -> Tile {
        let (cx, cy) = chunk_of(x, y);
        let c = self.chunk(cx, cy);
        Tile::from_u8(c.tiles[y.rem_euclid(CHUNK as i32) as usize * CHUNK + x.rem_euclid(CHUNK as i32) as usize])
    }

    pub fn walkable(&mut self, x: i32, y: i32) -> bool {
        self.tile(x, y).walkable()
    }

    /// Can a circle of radius `r` stand at `p`?
    pub fn fits(&mut self, p: Vec2, r: f32) -> bool {
        let (x0, x1) = ((p.x - r).floor() as i32, (p.x + r).floor() as i32);
        let (y0, y1) = ((p.y - r).floor() as i32, (p.y + r).floor() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if !self.walkable(x, y) {
                    return false;
                }
            }
        }
        true
    }

    /// Move with axis-separated collision so actors slide along walls.
    pub fn move_circle(&mut self, pos: &mut Vec2, delta: Vec2, r: f32) {
        // Split long moves so fast actors can't tunnel through 1-tile walls.
        let steps = (delta.length() / 0.4).ceil().max(1.0) as i32;
        let d = delta / steps as f32;
        for _ in 0..steps {
            let nx = vec2(pos.x + d.x, pos.y);
            if self.fits(nx, r) {
                *pos = nx;
            }
            let ny = vec2(pos.x, pos.y + d.y);
            if self.fits(ny, r) {
                *pos = ny;
            }
        }
    }

    /// Tile-stepped line of sight; walls block it.
    pub fn line_of_sight(&mut self, a: Vec2, b: Vec2) -> bool {
        let d = b - a;
        let steps = (d.length() * 2.0).ceil() as i32;
        for i in 1..steps {
            let p = a + d * (i as f32 / steps as f32);
            if self.tile(p.x.floor() as i32, p.y.floor() as i32).blocks_sight() {
                return false;
            }
        }
        true
    }

    /// Load chunks around `center` (in tiles), drop far ones, and return
    /// zombie spawns for chunks entering the active radius for the first time.
    pub fn stream(&mut self, center: Vec2, active_radius: i32, keep_radius: i32, avoid: Vec2) -> Vec<Vec2> {
        let (pcx, pcy) = chunk_of(center.x as i32, center.y as i32);
        self.chunks.retain(|&(cx, cy), _| (cx - pcx).abs() <= keep_radius && (cy - pcy).abs() <= keep_radius);
        let mut spawns = Vec::new();
        for cy in pcy - active_radius..=pcy + active_radius {
            for cx in pcx - active_radius..=pcx + active_radius {
                if !self.populated.insert((cx, cy)) {
                    continue;
                }
                let seed = self.seed;
                let tiles = self.chunk(cx, cy).tiles;
                for (x, y) in chunk_zombies(seed, cx, cy, &tiles) {
                    let p = vec2(x, y);
                    if p.distance(avoid) > 14.0 {
                        spawns.push(p);
                    }
                }
            }
        }
        spawns
    }

    /// Picks up loot within `r` tiles of `p`.
    pub fn take_loot_near(&mut self, p: Vec2, r: f32) -> Vec<Loot> {
        let (cx, cy) = chunk_of(p.x as i32, p.y as i32);
        let mut got = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let Some(c) = self.chunks.get_mut(&(cx + dx, cy + dy)) else { continue };
                c.loot.retain(|l| {
                    let near = vec2(l.x as f32 + 0.5, l.y as f32 + 0.5).distance(p) <= r;
                    if near {
                        got.push(*l);
                    }
                    !near
                });
            }
        }
        for l in &got {
            self.taken.insert((l.x, l.y));
        }
        got
    }

    /// Draw the visible chunks (world-space camera must be active).
    pub fn draw(&mut self, view: Rect) {
        let (cx0, cy0) = chunk_of(view.x.floor() as i32, view.y.floor() as i32);
        let (cx1, cy1) = chunk_of((view.x + view.w).ceil() as i32, (view.y + view.h).ceil() as i32);
        let size = CHUNK as f32;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let c = self.chunk(cx, cy);
                let tex = c.tex.get_or_insert_with(|| {
                    let mut rgba = Vec::with_capacity(CHUNK_TILES * 4);
                    for &t in c.tiles.iter() {
                        let [r, g, b] = Tile::from_u8(t).rgb();
                        rgba.extend_from_slice(&[r, g, b, 255]);
                    }
                    let tex = Texture2D::from_rgba8(CHUNK as u16, CHUNK as u16, &rgba);
                    tex.set_filter(FilterMode::Nearest);
                    tex
                });
                // Slight overlap hides hairline seams between chunk quads.
                draw_texture_ex(
                    tex,
                    cx as f32 * size,
                    cy as f32 * size,
                    WHITE,
                    DrawTextureParams { dest_size: Some(vec2(size + 0.02, size + 0.02)), ..Default::default() },
                );
                for l in &c.loot {
                    let col = loot_color(l.kind);
                    draw_rectangle(l.x as f32 + 0.25, l.y as f32 + 0.25, 0.5, 0.5, col);
                }
            }
        }
    }

    pub fn loaded_chunks(&self) -> usize {
        self.chunks.len()
    }
}

pub fn loot_color(k: crate::gen::LootKind) -> Color {
    use crate::gen::LootKind::*;
    match k {
        Food => Color::from_rgba(250, 170, 60, 255),
        Water => Color::from_rgba(120, 200, 255, 255),
        Medkit => Color::from_rgba(255, 255, 255, 255),
        Ammo => Color::from_rgba(240, 230, 110, 255),
        Scrap => Color::from_rgba(180, 180, 190, 255),
    }
}
