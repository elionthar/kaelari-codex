//! `.pkm` tile map format shared by the `osm2map` converter and the game.
//!
//! Layout (all integers little-endian):
//!
//! ```text
//! magic        b"PKMAP\0"
//! version      u16
//! width,height u32        map size in tiles (always a multiple of CHUNK)
//! m_per_tile   f32
//! bbox         4 x f64    south, west, north, east
//! stats        Stats      counts reported by the converter
//! labels       u32 count, then per label: x u16, y u16, kind u8, len u8, utf8
//! offsets      (chunks + 1) x u32 into the chunk blob, row-major chunk order
//! blob         per chunk: RLE pairs (run length u8, tile u8) covering CHUNK*CHUNK tiles
//! ```
//!
//! Chunks are stored independently so the game can decode only the ones near
//! the player.

use std::io::{self, Read, Write};

pub const MAGIC: &[u8; 6] = b"PKMAP\0";
pub const FORMAT_VERSION: u16 = 1;
pub const CHUNK: usize = 32;
pub const CHUNK_TILES: usize = CHUNK * CHUNK;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tile {
    Ground = 0,
    Grass = 1,
    Water = 2,
    Road = 3,
    Path = 4,
    Floor = 5,
    Wall = 6,
    Door = 7,
    Parking = 8,
    Rail = 9,
    Trees = 10,
    /// Padding outside the downloaded area.
    Edge = 11,
}

impl Tile {
    pub fn from_u8(v: u8) -> Tile {
        match v {
            0 => Tile::Ground,
            1 => Tile::Grass,
            2 => Tile::Water,
            3 => Tile::Road,
            4 => Tile::Path,
            5 => Tile::Floor,
            6 => Tile::Wall,
            7 => Tile::Door,
            8 => Tile::Parking,
            9 => Tile::Rail,
            10 => Tile::Trees,
            _ => Tile::Edge,
        }
    }

    pub fn walkable(self) -> bool {
        !matches!(self, Tile::Wall | Tile::Water | Tile::Edge)
    }

    pub fn blocks_sight(self) -> bool {
        matches!(self, Tile::Wall | Tile::Edge)
    }

    /// Placeholder color shared by the game and the converter's preview image.
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Tile::Ground => [86, 92, 70],
            Tile::Grass => [70, 128, 60],
            Tile::Water => [40, 80, 150],
            Tile::Road => [60, 60, 64],
            Tile::Path => [150, 140, 120],
            Tile::Floor => [120, 100, 80],
            Tile::Wall => [175, 85, 70],
            Tile::Door => [230, 200, 90],
            Tile::Parking => [95, 95, 100],
            Tile::Rail => [110, 80, 60],
            Tile::Trees => [45, 95, 45],
            Tile::Edge => [10, 10, 12],
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    Shop = 0,
    Amenity = 1,
    Building = 2,
}

impl LabelKind {
    pub fn from_u8(v: u8) -> LabelKind {
        match v {
            0 => LabelKind::Shop,
            1 => LabelKind::Amenity,
            _ => LabelKind::Building,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub x: u16,
    pub y: u16,
    pub kind: LabelKind,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub buildings: u32,
    pub roads: u32,
    pub footways: u32,
    pub road_km: f32,
    pub parks: u32,
    pub water: u32,
    pub pois: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

/// A map held in memory with chunk data still RLE-compressed.
pub struct PkMap {
    pub width: u32,
    pub height: u32,
    pub m_per_tile: f32,
    pub bbox: BBox,
    pub stats: Stats,
    pub labels: Vec<Label>,
    offsets: Vec<u32>,
    blob: Vec<u8>,
}

impl PkMap {
    pub fn chunks_x(&self) -> u32 {
        self.width / CHUNK as u32
    }

    pub fn chunks_y(&self) -> u32 {
        self.height / CHUNK as u32
    }

    /// Size of the compressed chunk data in bytes.
    pub fn blob_len(&self) -> usize {
        self.blob.len()
    }

    /// Decode one chunk into `out`. Chunks outside the map decode as `Edge`.
    pub fn decode_chunk(&self, cx: i32, cy: i32, out: &mut [u8; CHUNK_TILES]) {
        if cx < 0 || cy < 0 || cx as u32 >= self.chunks_x() || cy as u32 >= self.chunks_y() {
            out.fill(Tile::Edge as u8);
            return;
        }
        let i = (cy as u32 * self.chunks_x() + cx as u32) as usize;
        let data = &self.blob[self.offsets[i] as usize..self.offsets[i + 1] as usize];
        rle_decode(data, out);
    }

    /// Build a map from a full row-major tile grid. Width/height are padded up
    /// to a multiple of CHUNK with `Edge` tiles.
    pub fn from_tiles(
        width: usize,
        height: usize,
        tiles: &[u8],
        m_per_tile: f32,
        bbox: BBox,
        stats: Stats,
        labels: Vec<Label>,
    ) -> PkMap {
        assert_eq!(tiles.len(), width * height);
        let cxs = width.div_ceil(CHUNK);
        let cys = height.div_ceil(CHUNK);
        let mut offsets = Vec::with_capacity(cxs * cys + 1);
        let mut blob = Vec::new();
        let mut buf = [0u8; CHUNK_TILES];
        for cy in 0..cys {
            for cx in 0..cxs {
                for ly in 0..CHUNK {
                    for lx in 0..CHUNK {
                        let (x, y) = (cx * CHUNK + lx, cy * CHUNK + ly);
                        buf[ly * CHUNK + lx] = if x < width && y < height {
                            tiles[y * width + x]
                        } else {
                            Tile::Edge as u8
                        };
                    }
                }
                offsets.push(blob.len() as u32);
                rle_encode(&buf, &mut blob);
            }
        }
        offsets.push(blob.len() as u32);
        PkMap {
            width: (cxs * CHUNK) as u32,
            height: (cys * CHUNK) as u32,
            m_per_tile,
            bbox,
            stats,
            labels,
            offsets,
            blob,
        }
    }

    pub fn write_to(&self, w: &mut impl Write) -> io::Result<()> {
        w.write_all(MAGIC)?;
        w.write_all(&FORMAT_VERSION.to_le_bytes())?;
        w.write_all(&self.width.to_le_bytes())?;
        w.write_all(&self.height.to_le_bytes())?;
        w.write_all(&self.m_per_tile.to_le_bytes())?;
        for v in [self.bbox.south, self.bbox.west, self.bbox.north, self.bbox.east] {
            w.write_all(&v.to_le_bytes())?;
        }
        let s = &self.stats;
        for v in [s.buildings, s.roads, s.footways] {
            w.write_all(&v.to_le_bytes())?;
        }
        w.write_all(&s.road_km.to_le_bytes())?;
        for v in [s.parks, s.water, s.pois] {
            w.write_all(&v.to_le_bytes())?;
        }
        w.write_all(&(self.labels.len() as u32).to_le_bytes())?;
        for l in &self.labels {
            let name = truncate_utf8(&l.name, 255);
            w.write_all(&l.x.to_le_bytes())?;
            w.write_all(&l.y.to_le_bytes())?;
            w.write_all(&[l.kind as u8, name.len() as u8])?;
            w.write_all(name.as_bytes())?;
        }
        for o in &self.offsets {
            w.write_all(&o.to_le_bytes())?;
        }
        w.write_all(&self.blob)
    }

    pub fn read_from(r: &mut impl Read) -> io::Result<PkMap> {
        let mut magic = [0u8; 6];
        r.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(bad("not a .pkm file"));
        }
        let version = read_u16(r)?;
        if version != FORMAT_VERSION {
            return Err(bad(&format!("unsupported .pkm version {version}")));
        }
        let width = read_u32(r)?;
        let height = read_u32(r)?;
        if width % CHUNK as u32 != 0 || height % CHUNK as u32 != 0 || width == 0 || height == 0 {
            return Err(bad("map size must be a non-zero multiple of the chunk size"));
        }
        let m_per_tile = f32::from_bits(read_u32(r)?);
        let bbox = BBox {
            south: read_f64(r)?,
            west: read_f64(r)?,
            north: read_f64(r)?,
            east: read_f64(r)?,
        };
        let stats = Stats {
            buildings: read_u32(r)?,
            roads: read_u32(r)?,
            footways: read_u32(r)?,
            road_km: f32::from_bits(read_u32(r)?),
            parks: read_u32(r)?,
            water: read_u32(r)?,
            pois: read_u32(r)?,
        };
        let n_labels = read_u32(r)?;
        let mut labels = Vec::with_capacity(n_labels.min(100_000) as usize);
        for _ in 0..n_labels {
            let x = read_u16(r)?;
            let y = read_u16(r)?;
            let mut kl = [0u8; 2];
            r.read_exact(&mut kl)?;
            let mut name = vec![0u8; kl[1] as usize];
            r.read_exact(&mut name)?;
            labels.push(Label {
                x,
                y,
                kind: LabelKind::from_u8(kl[0]),
                name: String::from_utf8_lossy(&name).into_owned(),
            });
        }
        let n_chunks = (width / CHUNK as u32) as usize * (height / CHUNK as u32) as usize;
        let mut offsets = Vec::with_capacity(n_chunks + 1);
        for _ in 0..=n_chunks {
            offsets.push(read_u32(r)?);
        }
        let mut blob = Vec::new();
        r.read_to_end(&mut blob)?;
        if offsets.windows(2).any(|w| w[0] > w[1]) || *offsets.last().unwrap() as usize != blob.len() {
            return Err(bad("corrupt chunk offset table"));
        }
        Ok(PkMap { width, height, m_per_tile, bbox, stats, labels, offsets, blob })
    }
}

fn rle_encode(tiles: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < tiles.len() {
        let v = tiles[i];
        let mut run = 1;
        while i + run < tiles.len() && tiles[i + run] == v && run < 255 {
            run += 1;
        }
        out.push(run as u8);
        out.push(v);
        i += run;
    }
}

/// Decode RLE pairs; anything short or malformed is filled with `Edge`.
fn rle_decode(data: &[u8], out: &mut [u8; CHUNK_TILES]) {
    let mut pos = 0;
    for pair in data.chunks_exact(2) {
        let end = (pos + pair[0] as usize).min(CHUNK_TILES);
        out[pos..end].fill(pair[1]);
        pos = end;
    }
    out[pos..].fill(Tile::Edge as u8);
}

fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn read_u16(r: &mut impl Read) -> io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_f64(r: &mut impl Read) -> io::Result<f64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(f64::from_le_bytes(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let (w, h) = (40, 70);
        let tiles: Vec<u8> = (0..w * h).map(|i| ((i / 7) % 11) as u8).collect();
        let bbox = BBox { south: 1.0, west: 2.0, north: 3.0, east: 4.0 };
        let stats = Stats { buildings: 5, roads: 6, road_km: 1.5, ..Default::default() };
        let labels = vec![Label { x: 3, y: 4, kind: LabelKind::Shop, name: "Café ☕".into() }];
        let map = PkMap::from_tiles(w, h, &tiles, 2.0, bbox, stats, labels.clone());
        let mut bytes = Vec::new();
        map.write_to(&mut bytes).unwrap();
        let back = PkMap::read_from(&mut bytes.as_slice()).unwrap();
        assert_eq!((back.width, back.height), (64, 96));
        assert_eq!(back.stats, stats);
        assert_eq!(back.bbox, bbox);
        assert_eq!(back.labels, labels);
        let mut buf = [0u8; CHUNK_TILES];
        back.decode_chunk(1, 1, &mut buf);
        // Tile (33, 33) is inside the source grid.
        assert_eq!(buf[CHUNK + 1], tiles[33 * w + 33]);
        // Tile (40, 33) is padding.
        assert_eq!(buf[CHUNK + 8], Tile::Edge as u8);
        back.decode_chunk(-1, 0, &mut buf);
        assert!(buf.iter().all(|&t| t == Tile::Edge as u8));
    }
}
