//! Seeds: text seed -> u64 world seed, and per-system/per-chunk RNG streams.

use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Bump whenever generation logic changes so old seeds are known to differ.
pub const GEN_VERSION: u32 = 1;

pub fn seed_from_text(text: &str) -> u64 {
    let h = blake3::hash(text.as_bytes());
    u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap())
}

/// Independent RNG for one generation system (e.g. "loot") in one chunk.
pub fn chunk_rng(world_seed: u64, system: &str, cx: i32, cy: i32) -> ChaCha8Rng {
    let mut h = blake3::Hasher::new();
    h.update(b"poughkeepsie-chunk");
    h.update(&GEN_VERSION.to_le_bytes());
    h.update(&world_seed.to_le_bytes());
    h.update(&(system.len() as u32).to_le_bytes());
    h.update(system.as_bytes());
    h.update(&cx.to_le_bytes());
    h.update(&cy.to_le_bytes());
    ChaCha8Rng::from_seed(*h.finalize().as_bytes())
}

/// A readable seed like "rusty-trolley-4821" derived from the clock.
pub fn random_seed_text() -> String {
    const A: &[&str] = &["rusty", "quiet", "foggy", "burnt", "hollow", "cold", "silent", "broken", "grim", "pale", "ashen", "lost"];
    const B: &[&str] = &["walkway", "trolley", "bridge", "mall", "station", "river", "market", "chapel", "ferry", "arch", "depot", "harbor"];
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let h = blake3::hash(&nanos.to_le_bytes());
    let b = h.as_bytes();
    format!("{}-{}-{:04}", A[b[0] as usize % A.len()], B[b[1] as usize % B.len()], u16::from_le_bytes([b[2], b[3]]) % 10000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[test]
    fn deterministic_and_independent() {
        let s = seed_from_text("hudson");
        assert_eq!(s, seed_from_text("hudson"));
        assert_ne!(s, seed_from_text("Hudson"));
        let a = chunk_rng(s, "loot", 3, 4).next_u64();
        assert_eq!(a, chunk_rng(s, "loot", 3, 4).next_u64());
        assert_ne!(a, chunk_rng(s, "zombies", 3, 4).next_u64());
        assert_ne!(a, chunk_rng(s, "loot", 4, 3).next_u64());
        assert_ne!(a, chunk_rng(s + 1, "loot", 3, 4).next_u64());
    }
}
