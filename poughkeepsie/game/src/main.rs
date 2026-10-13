mod gen;
mod seed;
mod world;
mod zombie;

use std::path::PathBuf;

use ::rand::SeedableRng;
use macroquad::prelude::*;
use pkmap::{LabelKind, PkMap, Tile};
use rand_chacha::ChaCha8Rng;

use gen::LootKind;
use seed::{random_seed_text, seed_from_text, GEN_VERSION};
use world::World;
use zombie::{Zombie, ZOMBIE_R};

const PLAYER_R: f32 = 0.35;
const WALK: f32 = 3.0; // tiles per second (1 tile = 2 m)
const SPRINT: f32 = 4.6;
const ACTIVE_CHUNKS: i32 = 2; // zombies spawn/update within this many chunks
const KEEP_CHUNKS: i32 = 4; // chunks beyond this are unloaded

struct Args {
    seed: Option<String>,
    map: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut a = Args { seed: None, map: None };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--seed" => a.seed = it.next(),
            "--map" => a.map = it.next().map(PathBuf::from),
            "--fullscreen" => {} // read by window_conf
            s if s.starts_with("--seed=") => a.seed = Some(s["--seed=".len()..].to_string()),
            "-h" | "--help" => {
                println!("poughkeepsie [--seed TEXT] [--map PATH] [--fullscreen]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other} (try --help)");
                std::process::exit(2);
            }
        }
    }
    a
}

fn load_map(explicit: Option<PathBuf>) -> Result<PkMap, String> {
    let candidates: Vec<PathBuf> = match explicit {
        Some(p) => vec![p],
        None => {
            let mut v = vec![PathBuf::from("assets/poughkeepsie.pkm"), PathBuf::from("poughkeepsie/assets/poughkeepsie.pkm")];
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    v.push(dir.join("assets/poughkeepsie.pkm"));
                    v.push(dir.join("../../assets/poughkeepsie.pkm"));
                }
            }
            v.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/poughkeepsie.pkm")));
            v
        }
    };
    for p in &candidates {
        if let Ok(f) = std::fs::File::open(p) {
            return PkMap::read_from(&mut std::io::BufReader::new(f)).map_err(|e| format!("{}: {e}", p.display()));
        }
    }
    Err(format!("map not found (looked for {})", candidates[0].display()))
}

struct Game {
    world: World,
    seed_text: String,
    player: Vec2,
    facing: Vec2,
    hp: f32,
    zombies: Vec<Zombie>,
    inventory: [u32; 5],
    rng: ChaCha8Rng,
    time: f32,
    shove_cd: f32,
    tiles_tall: f32,
    message: Option<(String, f32)>,
}

impl Game {
    fn new(map: PkMap, seed_text: String) -> Game {
        let seed = seed_from_text(&seed_text);
        let mut world = World::new(map, seed);
        let player = find_spawn(&mut world);
        let rng = ChaCha8Rng::seed_from_u64(seed ^ 0x5eed_0f2a_0b1e);
        Game {
            world,
            seed_text,
            player,
            facing: vec2(1.0, 0.0),
            hp: 100.0,
            zombies: Vec::new(),
            inventory: [0; 5],
            rng,
            time: 0.0,
            shove_cd: 0.0,
            tiles_tall: 44.0,
            message: None,
        }
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        self.shove_cd = (self.shove_cd - dt).max(0.0);

        // Player movement.
        let mut input = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { input.y -= 1.0; }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { input.y += 1.0; }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { input.x -= 1.0; }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { input.x += 1.0; }
        if input != Vec2::ZERO {
            input = input.normalize();
            self.facing = input;
            let speed = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) { SPRINT } else { WALK };
            let mut p = self.player;
            self.world.move_circle(&mut p, input * speed * dt, PLAYER_R);
            self.player = p;
        }

        // Zoom.
        let wheel = mouse_wheel().1;
        if wheel != 0.0 || is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::Minus) {
            let zoom_in = wheel > 0.0 || is_key_pressed(KeyCode::Equal);
            self.tiles_tall = (self.tiles_tall * if zoom_in { 0.85 } else { 1.0 / 0.85 }).clamp(16.0, 160.0);
        }

        // Chunk streaming and seeded zombie spawns.
        for p in self.world.stream(self.player, ACTIVE_CHUNKS, KEEP_CHUNKS, self.player) {
            self.zombies.push(Zombie::new(p, &mut self.rng));
        }

        // Loot pickup.
        for l in self.world.take_loot_near(self.player, 0.8) {
            self.inventory[l.kind as usize] += 1;
            if l.kind == LootKind::Medkit && self.hp < 100.0 {
                self.hp = (self.hp + 25.0).min(100.0);
                self.inventory[l.kind as usize] -= 1;
                self.message = Some(("Used a medkit (+25 HP)".into(), 2.0));
            } else {
                self.message = Some((format!("Found {}", l.kind.name()), 1.5));
            }
        }

        // Shove: knock back nearby zombies.
        let shove = is_key_pressed(KeyCode::Space) && self.shove_cd == 0.0;
        if shove {
            self.shove_cd = 0.6;
        }

        let active = (ACTIVE_CHUNKS as f32 + 1.0) * pkmap::CHUNK as f32;
        let player = self.player;
        let mut damage = 0.0;
        for z in self.zombies.iter_mut() {
            let dist = player.distance(z.pos);
            if dist > active {
                continue; // frozen while far away
            }
            if shove && dist < 1.4 {
                z.shove(&mut self.world, player);
            }
            z.step(&mut self.world, player, dt, &mut self.rng);
            if dist < PLAYER_R + ZOMBIE_R + 0.1 {
                damage += 18.0 * dt;
            }
        }
        self.hp -= damage;
        if let Some((_, t)) = &mut self.message {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }
    }

    fn view(&self) -> Rect {
        let h = self.tiles_tall;
        let w = h * screen_width() / screen_height();
        Rect::new(self.player.x - w / 2.0, self.player.y - h / 2.0, w, h)
    }

    fn draw(&mut self) {
        let view = self.view();
        let cam = Camera2D::from_display_rect(view);
        set_camera(&cam);
        self.world.draw(view);
        for z in &self.zombies {
            if view.contains(z.pos) {
                let col = if z.chasing { Color::from_rgba(150, 230, 90, 255) } else { Color::from_rgba(90, 160, 70, 255) };
                draw_circle(z.pos.x, z.pos.y, ZOMBIE_R + 0.06, BLACK);
                draw_circle(z.pos.x, z.pos.y, ZOMBIE_R, col);
            }
        }
        draw_circle(self.player.x, self.player.y, PLAYER_R + 0.06, BLACK);
        draw_circle(self.player.x, self.player.y, PLAYER_R, Color::from_rgba(90, 170, 255, 255));
        let f = self.player + self.facing * 0.25;
        draw_circle(f.x, f.y, 0.1, WHITE);

        // Screen-space overlays.
        set_default_camera();
        let px_per_tile = screen_height() / self.tiles_tall;
        if px_per_tile >= 7.0 {
            for l in &self.world.map.labels {
                let p = vec2(l.x as f32 + 0.5, l.y as f32 + 0.5);
                if !view.contains(p) {
                    continue;
                }
                let s = cam.world_to_screen(p);
                let col = match l.kind {
                    LabelKind::Shop => Color::from_rgba(255, 220, 150, 255),
                    LabelKind::Amenity => Color::from_rgba(180, 220, 255, 255),
                    LabelKind::Building => Color::from_rgba(230, 230, 230, 200),
                };
                let dims = measure_text(&l.name, None, 16, 1.0);
                draw_rectangle(s.x - dims.width / 2.0 - 3.0, s.y - 13.0, dims.width + 6.0, 17.0, Color::new(0.0, 0.0, 0.0, 0.55));
                draw_text(&l.name, s.x - dims.width / 2.0, s.y, 16.0, col);
            }
        }
        self.draw_hud();
    }

    fn draw_hud(&self) {
        draw_rectangle(8.0, 8.0, 330.0, 92.0, Color::new(0.0, 0.0, 0.0, 0.6));
        draw_text(format!("Seed: {}", self.seed_text), 16.0, 28.0, 20.0, WHITE);
        draw_text(format!("gen v{GEN_VERSION}  {:016x}", self.world.seed), 16.0, 46.0, 16.0, GRAY);
        draw_rectangle(16.0, 56.0, 200.0, 14.0, Color::from_rgba(60, 20, 20, 255));
        draw_rectangle(16.0, 56.0, 2.0 * self.hp.max(0.0), 14.0, Color::from_rgba(200, 40, 40, 255));
        draw_text(format!("{:.0} HP", self.hp.max(0.0)), 224.0, 68.0, 18.0, WHITE);
        let inv: Vec<String> = LootKind::ALL.iter().map(|k| format!("{} {}", k.name(), self.inventory[*k as usize])).collect();
        draw_text(inv.join("  "), 16.0, 90.0, 16.0, LIGHTGRAY);

        let info = format!(
            "{} fps  chunks {}  zombies {}  ({:.0}, {:.0})",
            get_fps(),
            self.world.loaded_chunks(),
            self.zombies.len(),
            self.player.x,
            self.player.y
        );
        draw_text(&info, 8.0, screen_height() - 30.0, 16.0, GRAY);
        draw_text("WASD move  Shift sprint  Space shove  wheel/+/- zoom  Esc menu", 8.0, screen_height() - 12.0, 16.0, GRAY);
        if let Some((msg, _)) = &self.message {
            let d = measure_text(msg, None, 24, 1.0);
            draw_text(msg, (screen_width() - d.width) / 2.0, screen_height() * 0.75, 24.0, YELLOW);
        }
    }
}

/// A road tile near the middle of the map with room around it.
fn find_spawn(world: &mut World) -> Vec2 {
    let (cx, cy) = (world.map.width as i32 / 2, world.map.height as i32 / 2);
    for r in 0..400i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (cx + dx, cy + dy);
                if world.tile(x, y) == Tile::Road && world.fits(vec2(x as f32 + 0.5, y as f32 + 0.5), 1.0) {
                    return vec2(x as f32 + 0.5, y as f32 + 0.5);
                }
            }
        }
    }
    // No roads at all: any walkable tile.
    for y in 0..world.map.height as i32 {
        for x in 0..world.map.width as i32 {
            if world.fits(vec2(x as f32 + 0.5, y as f32 + 0.5), PLAYER_R) {
                return vec2(x as f32 + 0.5, y as f32 + 0.5);
            }
        }
    }
    vec2(cx as f32, cy as f32)
}

fn centered(text: &str, y: f32, size: u16, col: Color) {
    let d = measure_text(text, None, size, 1.0);
    draw_text(text, (screen_width() - d.width) / 2.0, y, size as f32, col);
}

enum Screen {
    Title,
    Playing,
    Dead,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Poughkeepsie Zombies".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        fullscreen: std::env::args().any(|a| a == "--fullscreen"),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let args = parse_args();
    let mut map = load_map(args.map.clone());
    let mut seed_text = args.seed.clone().unwrap_or_else(random_seed_text);
    let mut screen = Screen::Title;
    let mut game: Option<Game> = None;
    let mut stats_line = String::new();
    if let Ok(m) = &map {
        let s = m.stats;
        stats_line = format!(
            "{} buildings, {} roads ({:.1} km), {} footways, {} named places",
            s.buildings, s.roads, s.road_km, s.footways, s.pois
        );
    }
    // Headless screenshot hook for testing: POUGHKEEPSIE_SHOT=path.png[,frames]
    let shot = std::env::var("POUGHKEEPSIE_SHOT").ok();
    let mut frame = 0u32;
    if shot.is_some() && args.seed.is_some() && map.is_ok() {
        game = Some(Game::new(std::mem::replace(&mut map, Err(String::new())).unwrap(), seed_text.clone()));
        screen = Screen::Playing;
    }

    loop {
        let dt = get_frame_time().min(0.1);
        clear_background(Color::from_rgba(10, 10, 12, 255));
        match screen {
            Screen::Title => {
                while let Some(c) = get_char_pressed() {
                    if (c.is_alphanumeric() || c == '-' || c == '_' || c == ' ') && seed_text.len() < 40 {
                        seed_text.push(c);
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    seed_text.pop();
                }
                if is_key_pressed(KeyCode::Tab) {
                    seed_text = random_seed_text();
                }
                if is_key_pressed(KeyCode::Escape) {
                    break;
                }
                let h = screen_height();
                centered("DEAD IN POUGHKEEPSIE", h * 0.25, 64, Color::from_rgba(200, 50, 40, 255));
                centered("a top-down zombie survival game in downtown Poughkeepsie, NY", h * 0.25 + 40.0, 22, LIGHTGRAY);
                centered(&format!("Seed: {seed_text}_"), h * 0.48, 32, WHITE);
                centered(&format!("type to edit, Tab for random   (world {:016x}, gen v{GEN_VERSION})", seed_from_text(&seed_text)), h * 0.48 + 28.0, 18, GRAY);
                match &map {
                    Ok(_) => {
                        centered("Press Enter to start   Esc to quit", h * 0.64, 26, YELLOW);
                        centered(&stats_line, h * 0.64 + 30.0, 18, GRAY);
                    }
                    Err(e) => {
                        centered(e, h * 0.62, 20, RED);
                        centered("Build it with:  cargo run --release -p osm2map", h * 0.62 + 26.0, 20, RED);
                    }
                }
                centered("Map data \u{a9} OpenStreetMap contributors", h - 24.0, 20, LIGHTGRAY);

                if is_key_pressed(KeyCode::Enter) && map.is_ok() && !seed_text.trim().is_empty() {
                    let m = std::mem::replace(&mut map, Err(String::new())).unwrap();
                    game = Some(Game::new(m, seed_text.trim().to_string()));
                    screen = Screen::Playing;
                }
            }
            Screen::Playing => {
                let g = game.as_mut().unwrap();
                g.update(dt);
                g.draw();
                if g.hp <= 0.0 {
                    screen = Screen::Dead;
                }
                if is_key_pressed(KeyCode::Escape) {
                    map = Ok(game.take().unwrap().world.map);
                    screen = Screen::Title;
                }
            }
            Screen::Dead => {
                let g = game.as_mut().unwrap();
                g.draw();
                draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.3, 0.0, 0.0, 0.5));
                let h = screen_height();
                centered("YOU DIED", h * 0.4, 72, WHITE);
                let found: u32 = g.inventory.iter().sum();
                centered(&format!("Survived {:.0}:{:02.0}   items carried {found}", (g.time / 60.0).floor(), g.time % 60.0), h * 0.4 + 44.0, 24, LIGHTGRAY);
                centered("R: retry this seed   Enter: title", h * 0.4 + 80.0, 22, YELLOW);
                if is_key_pressed(KeyCode::R) {
                    let g = game.take().unwrap();
                    game = Some(Game::new(g.world.map, g.seed_text));
                    screen = Screen::Playing;
                } else if is_key_pressed(KeyCode::Enter) {
                    map = Ok(game.take().unwrap().world.map);
                    screen = Screen::Title;
                }
            }
        }

        if let Some(spec) = &shot {
            frame += 1;
            let mut parts = spec.split(',');
            let path = parts.next().unwrap_or("shot.png");
            let at: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(30);
            if frame == at {
                get_screen_data().export_png(path);
                break;
            }
        }
        next_frame().await;
    }
}
