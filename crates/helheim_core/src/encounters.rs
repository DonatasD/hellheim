use crate::enemies::Species;
use crate::map::NodeKind;
use crate::rng::RunRng;

use Species::*;

pub const WEAK_POOL: &[&[Species]] = &[
    &[BarrowRat],
    &[FenRat],
    &[GraveWolf],
    &[DraugrChanter],
    &[BarrowRat, FenRat],
];

pub const STRONG_POOL: &[&[Species]] = &[
    &[DraugrWarrior],
    &[MireCrawler, MireCrawler],
    &[Hrafn, GraveWolf],
    &[DraugrChanter, FenRat],
    &[BarrowRat, BarrowRat, FenRat],
    &[DraugrWarrior, MireCrawler],
];

pub const ELITE_POOL: &[&[Species]] = &[&[ForestTroll], &[BarrowWight], &[DraugrWarlord]];

/// Pick the enemy group for a node. `avoid` is the previous group; the roll is
/// re-rolled until it differs (pools have ≥2 entries, so this terminates).
pub fn roll_encounter(
    kind: NodeKind,
    floor: u8,
    rng: &mut RunRng,
    avoid: &[Species],
) -> Vec<Species> {
    let pool: &[&[Species]] = match kind {
        NodeKind::Boss => return vec![MoundJarl],
        NodeKind::Elite => ELITE_POOL,
        NodeKind::Monster if floor <= 3 => WEAK_POOL,
        NodeKind::Monster => STRONG_POOL,
        NodeKind::Rest | NodeKind::Treasure => return Vec::new(),
    };
    loop {
        let group = rng.pick(pool).to_vec();
        if group != avoid {
            return group;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::NodeKind;
    use crate::rng::RunRng;

    #[test]
    fn early_monsters_use_the_weak_pool() {
        let weak: Vec<Vec<Species>> = WEAK_POOL.iter().map(|g| g.to_vec()).collect();
        for seed in 0..30 {
            let mut rng = RunRng::new(seed);
            for floor in 1..=3 {
                let g = roll_encounter(NodeKind::Monster, floor, &mut rng, &[]);
                assert!(weak.contains(&g), "floor {floor} group {g:?} not weak");
            }
        }
    }

    #[test]
    fn late_monsters_use_the_strong_pool() {
        let strong: Vec<Vec<Species>> = STRONG_POOL.iter().map(|g| g.to_vec()).collect();
        for seed in 0..30 {
            let mut rng = RunRng::new(seed);
            let g = roll_encounter(NodeKind::Monster, 7, &mut rng, &[]);
            assert!(strong.contains(&g), "group {g:?} not strong");
        }
    }

    #[test]
    fn elites_and_boss_use_their_pools() {
        let mut rng = RunRng::new(1);
        let e = roll_encounter(NodeKind::Elite, 8, &mut rng, &[]);
        assert!(
            e == [Species::ForestTroll]
                || e == [Species::BarrowWight]
                || e == [Species::DraugrWarlord]
        );
        let b = roll_encounter(NodeKind::Boss, 16, &mut rng, &[]);
        assert_eq!(b, vec![Species::MoundJarl]);
    }

    #[test]
    fn never_repeats_the_avoided_group() {
        let mut rng = RunRng::new(9);
        let first = roll_encounter(NodeKind::Monster, 7, &mut rng, &[]);
        for _ in 0..40 {
            let g = roll_encounter(NodeKind::Monster, 7, &mut rng, &first);
            assert_ne!(g, first);
        }
    }

    #[test]
    fn deterministic_per_seed() {
        let mut a = RunRng::new(42);
        let mut b = RunRng::new(42);
        for floor in [1u8, 4, 7, 11] {
            assert_eq!(
                roll_encounter(NodeKind::Monster, floor, &mut a, &[]),
                roll_encounter(NodeKind::Monster, floor, &mut b, &[]),
            );
        }
    }

    #[test]
    fn rest_and_treasure_return_empty() {
        let mut rng = RunRng::new(0);
        assert!(roll_encounter(NodeKind::Rest, 5, &mut rng, &[]).is_empty());
        assert!(roll_encounter(NodeKind::Treasure, 9, &mut rng, &[]).is_empty());
    }
}
