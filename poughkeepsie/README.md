# Dead in Poughkeepsie

A top-down 2D zombie survival game set in downtown Poughkeepsie, NY, built from
real OpenStreetMap data. Rust + [macroquad](https://macroquad.rs). Built to run on
low-end hardware like a Pentium N4200 with 4 GB RAM.

Map data © OpenStreetMap contributors (ODbL).

## Quick start

```sh
cd poughkeepsie

# 1. Download downtown Poughkeepsie and build the map (needs internet, once)
cargo run --release -p osm2map

# 2. Play
cargo run --release -p game
cargo run --release -p game -- --seed "walkway at midnight"
```

The first build compiles dependencies and takes a few minutes on a slow CPU.
After that, `target/release/poughkeepsie` starts instantly.

Linux needs the usual X11/OpenGL runtime libraries (`libx11`, `libxi`, `libgl`),
which any desktop install already has. There is no audio yet, so ALSA is not needed.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | move |
| Shift | sprint |
| Space | shove nearby zombies (short stun) |
| Mouse wheel, `+` / `-` | zoom |
| Esc | back to title / quit |

On the title screen you can type a seed, or press Tab for a random one.

## Layout

| Crate | What it does |
| --- | --- |
| `pkmap` | The `.pkm` map format: 32×32 tile chunks, RLE-compressed separately, plus labels and data stats. No dependencies. |
| `osm2map` | Queries the Overpass API, caches the raw JSON in `data/`, rasterizes it at 2 m per tile and writes `assets/poughkeepsie.pkm`. Prints counts of buildings, roads, footways, parks, water features and named places so you can check the data is complete. Also writes `data/preview.ppm` (one pixel per tile). |
| `game` | The game. |

### osm2map

```sh
cargo run --release -p osm2map -- --help
cargo run --release -p osm2map -- --print-query     # the Overpass QL it sends
cargo run --release -p osm2map -- --refetch         # download again
```

The default bounding box is 41.6950,-73.9440 to 41.7125,-73.9160 (Hudson
waterfront and the Walkway east past Market St). That is about 2.3 × 1.9 km,
or 1184 × 992 tiles in 37 × 31 chunks.

Rasterization order: woods and parks, then parking and plazas, then water,
then rail, footways and roads (widths by road class), then buildings. Each
building gets a wall outline, a walkable floor and one door, preferably facing
a footway or road.

### Seeds

- Text seed → `u64`: the first 8 bytes of `blake3(text)`.
- Each generation system gets its own RNG per chunk: `ChaCha8Rng` seeded with
  `blake3(GEN_VERSION, world seed, system name, chunk x, chunk y)`. Loot and
  zombies use separate systems (`"loot"`, `"zombies"`), so adding a new system
  never shifts existing ones.
- `GEN_VERSION` (in `game/src/seed.rs`) is shown next to the seed. Bump it
  whenever generation changes, so the same seed is known to produce a
  different world.

### Performance notes

- The whole map stays compressed in memory (a few hundred KB). Only chunks near
  the player are decoded.
- Each loaded chunk becomes one 32×32 texture drawn as a single quad, so a
  frame draws about 20 quads for the map no matter how dense downtown is.
- Zombies more than 3 chunks away are frozen, and each zombie checks line of
  sight 5 times a second instead of every frame.

## Tests

```sh
cargo test --workspace
```

The converter test uses a small hand-written Overpass fixture
(`osm2map/tests/fixtures/tiny.json`). It is synthetic data, not real OSM data.
