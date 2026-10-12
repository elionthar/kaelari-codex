use osm2map::{convert, Overpass};
use pkmap::{BBox, LabelKind, Tile, CHUNK, CHUNK_TILES};

fn tiles(map: &pkmap::PkMap) -> Vec<u8> {
    let w = map.width as usize;
    let mut out = vec![0u8; w * map.height as usize];
    let mut buf = [0u8; CHUNK_TILES];
    for cy in 0..map.chunks_y() as i32 {
        for cx in 0..map.chunks_x() as i32 {
            map.decode_chunk(cx, cy, &mut buf);
            for (i, &t) in buf.iter().enumerate() {
                out[(cy as usize * CHUNK + i / CHUNK) * w + cx as usize * CHUNK + i % CHUNK] = t;
            }
        }
    }
    out
}

#[test]
fn converts_fixture() {
    let data: Overpass = serde_json::from_str(include_str!("fixtures/tiny.json")).unwrap();
    let bbox = BBox { south: 41.700, west: -73.930, north: 41.701, east: -73.928 };
    let (map, report) = convert(&data, bbox, 2.0);
    let s = report.stats;
    assert_eq!(s.buildings, 2);
    assert_eq!(report.building_relations, 1);
    assert_eq!(s.roads, 1);
    assert_eq!(s.footways, 1);
    assert_eq!(s.parks, 1);
    assert_eq!(s.water, 1);
    assert_eq!(s.pois, 2, "bakery deduped between node and building, plus the cafe");
    assert!(s.road_km > 0.18 && s.road_km < 0.22, "road_km {}", s.road_km);
    assert!(map.labels.iter().any(|l| l.kind == LabelKind::Building && l.name == "Courtyard Hall"));

    let t = tiles(&map);
    let count = |k: Tile| t.iter().filter(|&&v| v == k as u8).count();
    assert!(count(Tile::Road) > 50);
    assert!(count(Tile::Floor) > 20);
    assert!(count(Tile::Wall) > 20);
    assert_eq!(count(Tile::Door), 2, "one door per building");
    assert!(count(Tile::Grass) > 0 && count(Tile::Water) > 0 && count(Tile::Path) > 0);

    // The courtyard hole of the relation is not building.
    let w = map.width as usize;
    let (cx, cy) = (((-73.9284 + 73.930) * 111_320.0 * 41.7005f64.to_radians().cos() / 2.0) as usize, ((41.701 - 41.7006) * 110_574.0 / 2.0) as usize);
    let v = Tile::from_u8(t[cy * w + cx]);
    assert!(!matches!(v, Tile::Floor | Tile::Wall), "courtyard is {v:?}");
}
