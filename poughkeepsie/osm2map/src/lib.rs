//! Convert an Overpass JSON response (`out geom;`) into a `.pkm` tile map.

use std::collections::{HashMap, HashSet};

use pkmap::{BBox, Label, LabelKind, PkMap, Stats, Tile};
use serde::Deserialize;

/// Downtown Poughkeepsie: Hudson waterfront and the Walkway east to past Market St.
pub const DEFAULT_BBOX: BBox = BBox { south: 41.6950, west: -73.9440, north: 41.7125, east: -73.9160 };
pub const DEFAULT_M_PER_TILE: f32 = 2.0;

/// Overpass QL that fetches everything the converter understands.
pub fn overpass_query(b: &BBox) -> String {
    let bb = format!("({},{},{},{})", b.south, b.west, b.north, b.east);
    let parts = [
        "way[\"highway\"]",
        "way[\"building\"]",
        "relation[\"building\"]",
        "way[\"leisure\"~\"^(park|garden|playground|pitch|recreation_ground|dog_park)$\"]",
        "relation[\"leisure\"~\"^(park|garden|recreation_ground)$\"]",
        "way[\"landuse\"~\"^(grass|recreation_ground|cemetery|meadow|village_green|forest)$\"]",
        "relation[\"landuse\"~\"^(grass|recreation_ground|cemetery|forest)$\"]",
        "way[\"natural\"~\"^(water|wood|scrub)$\"]",
        "relation[\"natural\"~\"^(water|wood)$\"]",
        "way[\"waterway\"~\"^(riverbank|river|stream|canal)$\"]",
        "relation[\"waterway\"=\"riverbank\"]",
        "way[\"amenity\"=\"parking\"]",
        "way[\"railway\"~\"^(rail|light_rail)$\"]",
        "node[\"shop\"][\"name\"]",
        "node[\"amenity\"][\"name\"]",
        "way[\"shop\"][\"name\"]",
        "way[\"amenity\"][\"name\"]",
    ];
    let body: String = parts.iter().map(|p| format!("  {p}{bb};\n")).collect();
    format!("[out:json][timeout:240];\n(\n{body});\nout geom;\n")
}

#[derive(Deserialize)]
pub struct Overpass {
    pub elements: Vec<Element>,
}

#[derive(Deserialize)]
pub struct Element {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: i64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    #[serde(default)]
    pub geometry: Vec<Option<LatLon>>,
    #[serde(default)]
    pub members: Vec<Member>,
}

#[derive(Deserialize)]
pub struct Member {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub geometry: Vec<Option<LatLon>>,
}

#[derive(Deserialize, Clone, Copy, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

type Pt = (f64, f64);

/// Converts lat/lon to fractional tile coordinates (x east, y south).
struct Projection {
    west: f64,
    north: f64,
    sx: f64,
    sy: f64,
}

impl Projection {
    fn new(b: &BBox, m_per_tile: f32) -> Projection {
        let lat0 = ((b.south + b.north) / 2.0).to_radians();
        let m = m_per_tile as f64;
        Projection { west: b.west, north: b.north, sx: 111_320.0 * lat0.cos() / m, sy: 110_574.0 / m }
    }

    fn p(&self, ll: LatLon) -> Pt {
        ((ll.lon - self.west) * self.sx, (self.north - ll.lat) * self.sy)
    }
}

struct Grid {
    w: usize,
    h: usize,
    tiles: Vec<u8>,
}

impl Grid {
    fn set(&mut self, x: i64, y: i64, t: Tile) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.tiles[y as usize * self.w + x as usize] = t as u8;
        }
    }
}

/// Calls `f(x, y)` for every tile whose center lies inside the rings (even-odd rule,
/// so inner rings of multipolygons become holes).
fn fill_rings(rings: &[Vec<Pt>], w: usize, h: usize, mut f: impl FnMut(usize, usize)) {
    let (mut miny, mut maxy) = (f64::MAX, f64::MIN);
    for r in rings {
        for p in r {
            miny = miny.min(p.1);
            maxy = maxy.max(p.1);
        }
    }
    if rings.is_empty() || maxy < 0.0 || miny > h as f64 {
        return;
    }
    let y0 = miny.floor().max(0.0) as usize;
    let y1 = (maxy.ceil() as usize).min(h);
    let mut xs: Vec<f64> = Vec::new();
    for y in y0..y1 {
        let cy = y as f64 + 0.5;
        xs.clear();
        for r in rings {
            let n = r.len();
            for i in 0..n {
                let (a, b) = (r[i], r[(i + 1) % n]);
                if (a.1 <= cy) != (b.1 <= cy) {
                    xs.push(a.0 + (cy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            // Tiles whose center x+0.5 lies in [pair0, pair1).
            let xa = (pair[0] - 0.5).ceil().max(0.0);
            let xb = (pair[1] - 0.5).ceil().min(w as f64);
            let mut x = xa;
            while x < xb {
                f(x as usize, y);
                x += 1.0;
            }
        }
    }
}

/// Stamps a polyline `half` tiles wide on each side.
fn stroke(grid: &mut Grid, pts: &[Pt], half: f64, t: Tile) {
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let x0 = (a.0.min(b.0) - half).floor() as i64;
        let x1 = (a.0.max(b.0) + half).ceil() as i64;
        let y0 = (a.1.min(b.1) - half).floor() as i64;
        let y1 = (a.1.max(b.1) + half).ceil() as i64;
        if x1 < 0 || y1 < 0 || x0 > grid.w as i64 || y0 > grid.h as i64 {
            continue;
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dy * dy;
        for y in y0.max(0)..=y1.min(grid.h as i64 - 1) {
            for x in x0.max(0)..=x1.min(grid.w as i64 - 1) {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let u = if len2 > 0.0 { (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                let (qx, qy) = (a.0 + u * dx - px, a.1 + u * dy - py);
                if qx * qx + qy * qy <= half * half {
                    grid.set(x, y, t);
                }
            }
        }
    }
}

/// Joins multipolygon member ways into rings by matching endpoints.
fn assemble_rings(mut chains: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    let close = |a: Pt, b: Pt| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
    let mut rings = Vec::new();
    while let Some(mut cur) = chains.pop() {
        loop {
            if cur.len() > 2 && close(cur[0], *cur.last().unwrap()) {
                break;
            }
            let end = *cur.last().unwrap();
            let Some(i) = chains.iter().position(|c| close(c[0], end) || close(*c.last().unwrap(), end)) else {
                break; // unclosed: the fill closes it implicitly
            };
            let mut next = chains.swap_remove(i);
            if !close(next[0], end) {
                next.reverse();
            }
            cur.extend_from_slice(&next[1..]);
        }
        rings.push(cur);
    }
    rings
}

fn geom(g: &[Option<LatLon>], proj: &Projection) -> Vec<Pt> {
    g.iter().flatten().map(|&ll| proj.p(ll)).collect()
}

/// Polygon rings for a closed way or an area relation.
fn area_rings(e: &Element, proj: &Projection) -> Vec<Vec<Pt>> {
    if e.kind == "relation" {
        let chains: Vec<Vec<Pt>> = e
            .members
            .iter()
            .filter(|m| m.kind == "way" && (m.role == "outer" || m.role == "inner" || m.role.is_empty()))
            .map(|m| geom(&m.geometry, proj))
            .filter(|c| c.len() >= 2)
            .collect();
        assemble_rings(chains)
    } else {
        let r = geom(&e.geometry, proj);
        if r.len() >= 3 { vec![r] } else { vec![] }
    }
}

fn tag<'a>(e: &'a Element, k: &str) -> Option<&'a str> {
    e.tags.get(k).map(String::as_str)
}

fn is_closed(e: &Element) -> bool {
    let g: Vec<&LatLon> = e.geometry.iter().flatten().collect();
    g.len() >= 4 && g.first() == g.last()
}

/// Road width in meters for a highway value, or None if it isn't drawn.
fn highway_width(v: &str) -> Option<(f64, bool)> {
    // (width, is_footway)
    Some(match v {
        "motorway" | "trunk" | "primary" | "motorway_link" | "trunk_link" | "primary_link" => (13.0, false),
        "secondary" | "secondary_link" => (11.0, false),
        "tertiary" | "tertiary_link" => (9.0, false),
        "residential" | "unclassified" | "living_street" | "road" => (7.0, false),
        "service" => (4.0, false),
        "pedestrian" | "footway" | "path" | "steps" | "cycleway" | "bridleway" | "track" | "corridor" => (2.5, true),
        _ => return None,
    })
}

pub struct Report {
    pub stats: Stats,
    pub elements: usize,
    pub building_relations: u32,
    pub size_tiles: (u32, u32),
    pub chunks: (u32, u32),
    pub compressed_bytes: usize,
    pub tile_counts: [u64; 12],
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = &self.stats;
        writeln!(f, "Overpass elements:   {}", self.elements)?;
        writeln!(f, "Buildings:           {} ({} multipolygon relations)", s.buildings, self.building_relations)?;
        writeln!(f, "Roads (car):         {}", s.roads)?;
        writeln!(f, "Footways/paths:      {}", s.footways)?;
        writeln!(f, "Road length:         {:.1} km (inside the bbox)", s.road_km)?;
        writeln!(f, "Parks/green areas:   {}", s.parks)?;
        writeln!(f, "Water features:      {}", s.water)?;
        writeln!(f, "Named shops/amenity: {}", s.pois)?;
        writeln!(f, "Map size:            {} x {} tiles, {} x {} chunks", self.size_tiles.0, self.size_tiles.1, self.chunks.0, self.chunks.1)?;
        writeln!(f, "Compressed tiles:    {} KiB", self.compressed_bytes / 1024)?;
        let total: u64 = self.tile_counts.iter().sum();
        write!(f, "Tile mix:           ")?;
        for (i, &n) in self.tile_counts.iter().enumerate() {
            if n > 0 {
                write!(f, " {:?} {:.1}%", Tile::from_u8(i as u8), n as f64 * 100.0 / total as f64)?;
            }
        }
        Ok(())
    }
}

pub fn convert(data: &Overpass, bbox: BBox, m_per_tile: f32) -> (PkMap, Report) {
    let proj = Projection::new(&bbox, m_per_tile);
    let (fw, fh) = proj.p(LatLon { lat: bbox.south, lon: bbox.east });
    let (w, h) = (fw.ceil() as usize, fh.ceil() as usize);
    let mut grid = Grid { w, h, tiles: vec![Tile::Ground as u8; w * h] };
    let mut stats = Stats::default();
    let mut building_relations = 0;

    // Layer order: later layers overwrite earlier ones.
    let mut seen_way = HashSet::new();
    let els: Vec<&Element> = data
        .elements
        .iter()
        .filter(|e| e.kind != "way" || seen_way.insert(e.id)) // Overpass can repeat ways across union members
        .collect();

    // 1. Woods and green areas.
    for e in &els {
        let woods = matches!(tag(e, "natural"), Some("wood" | "scrub")) || tag(e, "landuse") == Some("forest");
        let green = matches!(tag(e, "leisure"), Some("park" | "garden" | "playground" | "pitch" | "recreation_ground" | "dog_park"))
            || matches!(tag(e, "landuse"), Some("grass" | "recreation_ground" | "cemetery" | "meadow" | "village_green"));
        if (woods || green) && e.kind != "node" && (e.kind == "relation" || is_closed(e)) {
            let t = if woods { Tile::Trees } else { Tile::Grass };
            fill_rings(&area_rings(e, &proj), w, h, |x, y| grid.tiles[y * w + x] = t as u8);
            if green && tag(e, "leisure").is_some() {
                stats.parks += 1;
            }
        }
    }
    // 2. Parking lots and pedestrian plazas.
    for e in &els {
        let parking = tag(e, "amenity") == Some("parking") && e.kind == "way" && is_closed(e);
        let plaza = tag(e, "highway") == Some("pedestrian") && tag(e, "area") == Some("yes") && is_closed(e);
        if parking || plaza {
            let t = if parking { Tile::Parking } else { Tile::Path };
            fill_rings(&area_rings(e, &proj), w, h, |x, y| grid.tiles[y * w + x] = t as u8);
        }
    }
    // 3. Water: areas, then streams/rivers as lines.
    for e in &els {
        let area = tag(e, "natural") == Some("water") || tag(e, "waterway") == Some("riverbank");
        if area && e.kind != "node" && (e.kind == "relation" || is_closed(e)) {
            fill_rings(&area_rings(e, &proj), w, h, |x, y| grid.tiles[y * w + x] = Tile::Water as u8);
            stats.water += 1;
        } else if e.kind == "way" && tag(e, "tunnel").is_none() {
            let width = match tag(e, "waterway") {
                Some("river") => 25.0,
                Some("canal") => 8.0,
                Some("stream") => 4.0,
                _ => continue,
            };
            stroke(&mut grid, &geom(&e.geometry, &proj), (width / 2.0 / m_per_tile as f64).max(0.75), Tile::Water);
            stats.water += 1;
        }
    }
    // 4. Rail, footways, then roads (wider roads drawn last so they win at junctions).
    let mut road_ways: Vec<(f64, Vec<Pt>)> = Vec::new();
    for e in &els {
        if e.kind != "way" {
            continue;
        }
        let pts = geom(&e.geometry, &proj);
        if matches!(tag(e, "railway"), Some("rail" | "light_rail")) && tag(e, "tunnel").is_none() {
            stroke(&mut grid, &pts, 1.0, Tile::Rail);
        }
        let Some(hw) = tag(e, "highway") else { continue };
        if tag(e, "area") == Some("yes") {
            continue;
        }
        let Some((width, foot)) = highway_width(hw) else { continue };
        if foot {
            stats.footways += 1;
            stroke(&mut grid, &pts, (width / 2.0 / m_per_tile as f64).max(0.75), Tile::Path);
        } else {
            stats.roads += 1;
            stats.road_km += clipped_len_m(&pts, w, h, m_per_tile) as f32 / 1000.0;
            road_ways.push((width, pts));
        }
    }
    road_ways.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (width, pts) in &road_ways {
        stroke(&mut grid, pts, (width / 2.0 / m_per_tile as f64).max(0.75), Tile::Road);
    }

    // 5. Buildings: fill with an id per building, then walls on the outline and one door each.
    let mut owner = vec![0u32; w * h];
    let mut next_id = 0u32;
    for e in &els {
        if e.kind == "node" || !e.tags.contains_key("building") || tag(e, "building") == Some("no") {
            continue;
        }
        if e.kind == "way" && !is_closed(e) {
            continue;
        }
        let rings = area_rings(e, &proj);
        if rings.is_empty() {
            continue;
        }
        stats.buildings += 1;
        if e.kind == "relation" {
            building_relations += 1;
        }
        next_id += 1;
        let id = next_id;
        fill_rings(&rings, w, h, |x, y| owner[y * w + x] = id);
    }
    let at = |x: i64, y: i64| -> u32 {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { 0 } else { owner[y as usize * w + x as usize] }
    };
    const N4: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    for y in 0..h {
        for x in 0..w {
            let id = owner[y * w + x];
            if id == 0 {
                continue;
            }
            let edge = N4.iter().any(|&(dx, dy)| at(x as i64 + dx, y as i64 + dy) != id);
            grid.tiles[y * w + x] = if edge { Tile::Wall } else { Tile::Floor } as u8;
        }
    }
    // A door is a wall tile with outside on one side and its own floor on the other.
    // Prefer doors facing footways, then roads.
    let mut doors: HashMap<u32, (u8, usize)> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let id = owner[i];
            if id == 0 || grid.tiles[i] != Tile::Wall as u8 {
                continue;
            }
            for &(dx, dy) in &N4 {
                let (ox, oy, ix, iy) = (x as i64 + dx, y as i64 + dy, x as i64 - dx, y as i64 - dy);
                if at(ox, oy) != 0 || at(ix, iy) != id || ox < 0 || oy < 0 || ox >= w as i64 || oy >= h as i64 {
                    continue;
                }
                if grid.tiles[iy as usize * w + ix as usize] != Tile::Floor as u8 {
                    continue;
                }
                let score = match Tile::from_u8(grid.tiles[oy as usize * w + ox as usize]) {
                    Tile::Path => 4,
                    Tile::Road => 3,
                    Tile::Parking | Tile::Ground | Tile::Grass => 2,
                    Tile::Water | Tile::Edge => 0,
                    _ => 1,
                };
                if score > 0 && doors.get(&id).is_none_or(|&(s, _)| score > s) {
                    doors.insert(id, (score, i));
                }
            }
        }
    }
    for &(_, i) in doors.values() {
        grid.tiles[i] = Tile::Door as u8;
    }

    // 6. Labels for named shops, amenities and buildings.
    let mut labels = Vec::new();
    let mut seen_label = HashSet::new();
    for e in &els {
        let Some(name) = e.tags.get("name") else { continue };
        let kind = if e.tags.contains_key("shop") {
            LabelKind::Shop
        } else if e.tags.contains_key("amenity") {
            LabelKind::Amenity
        } else if e.tags.contains_key("building") && e.kind != "node" {
            LabelKind::Building
        } else {
            continue;
        };
        let p = match (e.lat, e.lon) {
            (Some(lat), Some(lon)) => proj.p(LatLon { lat, lon }),
            _ => {
                let pts: Vec<Pt> = area_rings(e, &proj).into_iter().flatten().collect();
                if pts.is_empty() {
                    continue;
                }
                let n = pts.len() as f64;
                (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n)
            }
        };
        if p.0 < 0.0 || p.1 < 0.0 || p.0 >= w as f64 || p.1 >= h as f64 {
            continue;
        }
        let (x, y) = (p.0 as u16, p.1 as u16);
        if !seen_label.insert((name.clone(), x / 8, y / 8)) {
            continue;
        }
        if kind != LabelKind::Building {
            stats.pois += 1;
        }
        labels.push(Label { x, y, kind, name: name.clone() });
    }
    labels.sort_by_key(|l| (l.y, l.x));

    let mut tile_counts = [0u64; 12];
    for &t in &grid.tiles {
        tile_counts[(t as usize).min(11)] += 1;
    }
    let map = PkMap::from_tiles(w, h, &grid.tiles, m_per_tile, bbox, stats, labels);
    let report = Report {
        stats,
        elements: data.elements.len(),
        building_relations,
        size_tiles: (map.width, map.height),
        chunks: (map.chunks_x(), map.chunks_y()),
        compressed_bytes: map.blob_len(),
        tile_counts,
    };
    (map, report)
}

/// Length of the parts of a polyline inside the grid, in meters (approximate:
/// a segment counts if its midpoint is inside).
fn clipped_len_m(pts: &[Pt], w: usize, h: usize, m_per_tile: f32) -> f64 {
    pts.windows(2)
        .filter(|s| {
            let (mx, my) = ((s[0].0 + s[1].0) / 2.0, (s[0].1 + s[1].1) / 2.0);
            mx >= 0.0 && my >= 0.0 && mx < w as f64 && my < h as f64
        })
        .map(|s| ((s[1].0 - s[0].0).powi(2) + (s[1].1 - s[0].1).powi(2)).sqrt() * m_per_tile as f64)
        .sum()
}

/// Write a binary PPM preview, one pixel per tile.
pub fn write_ppm(map: &PkMap, path: &std::path::Path) -> std::io::Result<()> {
    let (w, h) = (map.width as usize, map.height as usize);
    let mut px = vec![0u8; w * h * 3];
    let mut buf = [0u8; pkmap::CHUNK_TILES];
    for cy in 0..map.chunks_y() as i32 {
        for cx in 0..map.chunks_x() as i32 {
            map.decode_chunk(cx, cy, &mut buf);
            for (i, &t) in buf.iter().enumerate() {
                let x = cx as usize * pkmap::CHUNK + i % pkmap::CHUNK;
                let y = cy as usize * pkmap::CHUNK + i / pkmap::CHUNK;
                px[(y * w + x) * 3..][..3].copy_from_slice(&Tile::from_u8(t).rgb());
            }
        }
    }
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    out.extend_from_slice(&px);
    std::fs::write(path, out)
}
