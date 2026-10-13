//! Zombie behavior: wander aimlessly, chase the player on sight, then search
//! where they were last seen for a few seconds.

use macroquad::math::{vec2, Vec2};
use rand::Rng;

use crate::world::World;

pub const ZOMBIE_R: f32 = 0.35;
const WANDER_SPEED: f32 = 0.7; // tiles per second
const CHASE_SPEED: f32 = 2.6;
const SIGHT: f32 = 16.0; // tiles
const MEMORY: f32 = 4.0; // seconds spent heading to the last seen spot

pub struct Zombie {
    pub pos: Vec2,
    pub chasing: bool,
    pub stun: f32,
    dir: Vec2,
    think: f32,
    sight_check: f32,
    last_seen: Vec2,
    memory: f32,
}

impl Zombie {
    pub fn new(pos: Vec2, rng: &mut impl Rng) -> Zombie {
        Zombie {
            pos,
            chasing: false,
            stun: 0.0,
            dir: Vec2::ZERO,
            think: 0.0,
            // Stagger sight checks so they don't all run on the same frame.
            sight_check: rng.gen_range(0.0..0.2),
            last_seen: pos,
            memory: 0.0,
        }
    }

    pub fn shove(&mut self, world: &mut World, from: Vec2) {
        let away = (self.pos - from).try_normalize().unwrap_or(vec2(1.0, 0.0));
        world.move_circle(&mut self.pos, away * 1.5, ZOMBIE_R);
        self.stun = 1.0;
    }

    pub fn step(&mut self, world: &mut World, player: Vec2, dt: f32, rng: &mut impl Rng) {
        if self.stun > 0.0 {
            self.stun -= dt;
            return;
        }
        self.sight_check -= dt;
        if self.sight_check <= 0.0 {
            self.sight_check = 0.2;
            if self.pos.distance(player) < SIGHT && world.line_of_sight(self.pos, player) {
                self.chasing = true;
                self.last_seen = player;
                self.memory = MEMORY;
            }
        }
        let (dir, speed) = if self.chasing {
            self.memory -= dt;
            let to = self.last_seen - self.pos;
            if self.memory <= 0.0 || to.length() < 0.3 {
                self.chasing = false;
            }
            (to.normalize_or_zero(), CHASE_SPEED)
        } else {
            self.think -= dt;
            if self.think <= 0.0 {
                self.think = rng.gen_range(1.5..4.0);
                self.dir = if rng.gen_bool(0.3) {
                    Vec2::ZERO
                } else {
                    Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU))
                };
            }
            (self.dir, WANDER_SPEED)
        };
        let before = self.pos;
        world.move_circle(&mut self.pos, dir * speed * dt, ZOMBIE_R);
        if !self.chasing && self.pos == before {
            self.think = 0.0; // bumped into something: pick a new direction
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkmap::{BBox, PkMap, Stats, Tile};
    use rand::SeedableRng;

    fn world_with_wall(wall: bool) -> World {
        let (w, h) = (64, 64);
        let mut tiles = vec![Tile::Road as u8; w * h];
        if wall {
            for y in 0..h {
                tiles[y * w + 20] = Tile::Wall as u8;
            }
        }
        let bbox = BBox { south: 0.0, west: 0.0, north: 1.0, east: 1.0 };
        World::new(PkMap::from_tiles(w, h, &tiles, 2.0, bbox, Stats::default(), vec![]), 1)
    }

    fn run(world: &mut World, z: &mut Zombie, player: Vec2, secs: f32) {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        for _ in 0..(secs * 60.0) as i32 {
            z.step(world, player, 1.0 / 60.0, &mut rng);
        }
    }

    #[test]
    fn chases_when_visible() {
        let mut world = world_with_wall(false);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
        let mut z = Zombie::new(vec2(10.5, 30.5), &mut rng);
        let player = vec2(22.5, 30.5);
        run(&mut world, &mut z, player, 6.0);
        assert!(z.pos.distance(player) < 1.0, "zombie at {:?}", z.pos);
    }

    #[test]
    fn walls_block_sight() {
        let mut world = world_with_wall(true);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
        let mut z = Zombie::new(vec2(10.5, 30.5), &mut rng);
        run(&mut world, &mut z, vec2(22.5, 30.5), 2.0);
        assert!(!z.chasing);
        assert!(z.pos.x < 20.0);
    }

    #[test]
    fn ignores_distant_player() {
        let mut world = world_with_wall(false);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
        let mut z = Zombie::new(vec2(5.5, 5.5), &mut rng);
        run(&mut world, &mut z, vec2(50.5, 50.5), 1.0);
        assert!(!z.chasing);
    }
}
