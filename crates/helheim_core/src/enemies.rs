#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    DraugrChanter, // StS Cultist
    GraveWolf,     // StS Jaw Worm
    BarrowRat,     // StS Red Louse
    FenRat,        // StS Green Louse
    ForestTroll,   // StS Gremlin Nob (elite)
    // Act 1 map bestiary:
    DraugrWarrior, // StS Blue Slaver
    MireCrawler,   // StS Fungi Beast
    Hrafn,         // carrion crow
    BarrowWight,   // StS Lagavulin (elite)
    DraugrWarlord, // elite
    MoundJarl,     // Act 1 boss
}

impl Species {
    pub fn name(self) -> &'static str {
        match self {
            Species::DraugrChanter => "Draugr Chanter",
            Species::GraveWolf => "Grave Wolf",
            Species::BarrowRat => "Barrow Rat",
            Species::FenRat => "Fen Rat",
            Species::ForestTroll => "Forest Troll",
            Species::DraugrWarrior => "Draugr Warrior",
            Species::MireCrawler => "Mire Crawler",
            Species::Hrafn => "Hrafn",
            Species::BarrowWight => "Barrow Wight",
            Species::DraugrWarlord => "Draugr Warlord",
            Species::MoundJarl => "The Mound Jarl",
        }
    }

    /// (min, max) inclusive, rolled at spawn.
    pub fn hp_range(self) -> (u32, u32) {
        match self {
            Species::DraugrChanter => (48, 54),
            Species::GraveWolf => (40, 44),
            Species::BarrowRat => (10, 15),
            Species::FenRat => (11, 17),
            Species::ForestTroll => (82, 86),
            Species::DraugrWarrior => (46, 50),
            Species::MireCrawler => (22, 28),
            Species::Hrafn => (30, 34),
            Species::BarrowWight => (85, 90),
            Species::DraugrWarlord => (86, 90),
            Species::MoundJarl => (150, 150),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyMove {
    // Draugr Chanter
    Chant,      // gain Ritual 3
    DarkStrike, // attack 6
    // Grave Wolf
    Chomp,  // attack 11
    Thrash, // attack 7, gain 5 block
    Bellow, // +3 Strength, +6 block
    // Rats
    Bite,    // attack (rolled 5–7 at spawn)
    Grow,    // +3 Strength (Barrow Rat)
    Spittle, // apply 2 Weak (Fen Rat)
    // Forest Troll
    TrollBellow, // gain Enrage 2
    Rush,        // attack 14
    SkullBash,   // attack 6, apply 2 Vulnerable
    // Act 1 bestiary moves:
    Stab,         // attack 12
    Rend,         // attack 8 + Weak 1
    Fester,       // gain Strength 4
    Peck,         // attack 4 x2
    Screech,      // attack 5 + Vulnerable 1
    Maul,         // attack 18
    SoulDrain,    // player -2 Strength
    WarChant,     // gain Ritual 2
    Cleave,       // attack 8 x2
    CrushingBlow, // attack 22
    GraveCleave,  // attack 6 x3
    DreadRoar,    // Strength +3 + player Vulnerable 2
    Bulwark,      // Block 18 + attack 10
}

impl EnemyMove {
    pub fn name(self) -> &'static str {
        match self {
            EnemyMove::Chant => "Chant",
            EnemyMove::DarkStrike => "Dark Strike",
            EnemyMove::Chomp => "Chomp",
            EnemyMove::Thrash => "Thrash",
            EnemyMove::Bellow => "Bellow",
            EnemyMove::Bite => "Bite",
            EnemyMove::Grow => "Grow",
            EnemyMove::Spittle => "Diseased Spittle",
            EnemyMove::TrollBellow => "Bellow",
            EnemyMove::Rush => "Rush",
            EnemyMove::SkullBash => "Skull Bash",
            EnemyMove::Stab => "Stab",
            EnemyMove::Rend => "Rend",
            EnemyMove::Fester => "Fester",
            EnemyMove::Peck => "Peck",
            EnemyMove::Screech => "Screech",
            EnemyMove::Maul => "Maul",
            EnemyMove::SoulDrain => "Soul Drain",
            EnemyMove::WarChant => "War-Chant",
            EnemyMove::Cleave => "Cleave",
            EnemyMove::CrushingBlow => "Crushing Blow",
            EnemyMove::GraveCleave => "Grave Cleave",
            EnemyMove::DreadRoar => "Dread Roar",
            EnemyMove::Bulwark => "Bulwark",
        }
    }
}

use crate::rng::RunRng;

fn ran_consecutively(history: &[EnemyMove], mv: EnemyMove, times: usize) -> bool {
    history.len() >= times && history[history.len() - times..].iter().all(|m| *m == mv)
}

/// Pick the enemy's next move per its StS pattern. Re-rolls on no-repeat
/// violations (a legal move always exists in these rule sets).
pub fn roll_move(species: Species, history: &[EnemyMove], rng: &mut RunRng) -> EnemyMove {
    let first_turn = history.is_empty();
    match species {
        Species::DraugrChanter => {
            if first_turn {
                EnemyMove::Chant
            } else {
                EnemyMove::DarkStrike
            }
        }
        Species::GraveWolf => {
            if first_turn {
                return EnemyMove::Chomp;
            }
            loop {
                let roll = rng.percent();
                let candidate = if roll < 25 {
                    EnemyMove::Chomp
                } else if roll < 55 {
                    EnemyMove::Thrash
                } else {
                    EnemyMove::Bellow
                };
                let ok = match candidate {
                    EnemyMove::Chomp => !ran_consecutively(history, EnemyMove::Chomp, 1),
                    EnemyMove::Bellow => !ran_consecutively(history, EnemyMove::Bellow, 1),
                    EnemyMove::Thrash => !ran_consecutively(history, EnemyMove::Thrash, 2),
                    _ => unreachable!(),
                };
                if ok {
                    return candidate;
                }
            }
        }
        Species::BarrowRat | Species::FenRat => {
            let special = if species == Species::BarrowRat {
                EnemyMove::Grow
            } else {
                EnemyMove::Spittle
            };
            loop {
                let candidate = if rng.percent() < 75 {
                    EnemyMove::Bite
                } else {
                    special
                };
                let ok = if candidate == EnemyMove::Bite {
                    !ran_consecutively(history, EnemyMove::Bite, 2)
                } else {
                    !ran_consecutively(history, special, 1)
                };
                if ok {
                    return candidate;
                }
            }
        }
        Species::ForestTroll => {
            if first_turn {
                return EnemyMove::TrollBellow;
            }
            loop {
                let candidate = if rng.percent() < 33 {
                    EnemyMove::SkullBash
                } else {
                    EnemyMove::Rush
                };
                if candidate == EnemyMove::Rush && ran_consecutively(history, EnemyMove::Rush, 2) {
                    continue;
                }
                return candidate;
            }
        }
        Species::DraugrWarrior => loop {
            let candidate = if rng.percent() < 60 {
                EnemyMove::Stab
            } else {
                EnemyMove::Rend
            };
            if ran_consecutively(history, candidate, 2) {
                continue;
            }
            return candidate;
        },
        Species::Hrafn => loop {
            let candidate = if rng.percent() < 60 {
                EnemyMove::Peck
            } else {
                EnemyMove::Screech
            };
            if ran_consecutively(history, candidate, 2) {
                continue;
            }
            return candidate;
        },
        Species::MireCrawler => {
            if first_turn {
                return EnemyMove::Fester;
            }
            loop {
                let candidate = if rng.percent() < 70 {
                    EnemyMove::Bite
                } else {
                    EnemyMove::Fester
                };
                if candidate == EnemyMove::Fester
                    && ran_consecutively(history, EnemyMove::Fester, 1)
                {
                    continue;
                }
                return candidate;
            }
        }
        Species::BarrowWight => {
            if first_turn {
                return EnemyMove::SoulDrain;
            }
            loop {
                let candidate = if rng.percent() < 70 {
                    EnemyMove::Maul
                } else {
                    EnemyMove::SoulDrain
                };
                if candidate == EnemyMove::SoulDrain
                    && ran_consecutively(history, EnemyMove::SoulDrain, 1)
                {
                    continue;
                }
                return candidate;
            }
        }
        Species::DraugrWarlord => {
            if first_turn {
                EnemyMove::WarChant
            } else {
                EnemyMove::Cleave
            }
        }
        Species::MoundJarl => {
            use EnemyMove::*;
            [DreadRoar, CrushingBlow, GraveCleave, Bulwark][history.len() % 4]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_data_matches_spec() {
        assert_eq!(Species::DraugrChanter.name(), "Draugr Chanter");
        assert_eq!(Species::DraugrChanter.hp_range(), (48, 54));
        assert_eq!(Species::GraveWolf.hp_range(), (40, 44));
        assert_eq!(Species::BarrowRat.hp_range(), (10, 15));
        assert_eq!(Species::FenRat.hp_range(), (11, 17));
        assert_eq!(Species::ForestTroll.hp_range(), (82, 86));
    }

    use crate::rng::RunRng;

    /// Simulate `n` turns of one enemy's move history.
    fn simulate(species: Species, n: usize, seed: u64) -> Vec<EnemyMove> {
        let mut rng = RunRng::new(seed);
        let mut history = Vec::new();
        for _ in 0..n {
            let mv = roll_move(species, &history, &mut rng);
            history.push(mv);
        }
        history
    }

    fn max_consecutive(history: &[EnemyMove], mv: EnemyMove) -> usize {
        let mut best = 0;
        let mut cur = 0;
        for &m in history {
            cur = if m == mv { cur + 1 } else { 0 };
            best = best.max(cur);
        }
        best
    }

    #[test]
    fn chanter_chants_once_then_strikes_forever() {
        let h = simulate(Species::DraugrChanter, 10, 1);
        assert_eq!(h[0], EnemyMove::Chant);
        assert!(h[1..].iter().all(|m| *m == EnemyMove::DarkStrike));
    }

    #[test]
    fn wolf_always_opens_with_chomp() {
        for seed in 0..20 {
            assert_eq!(simulate(Species::GraveWolf, 1, seed)[0], EnemyMove::Chomp);
        }
    }

    #[test]
    fn wolf_respects_repeat_rules_and_uses_all_moves() {
        for seed in 0..10 {
            let h = simulate(Species::GraveWolf, 300, seed);
            assert!(max_consecutive(&h, EnemyMove::Chomp) <= 1);
            assert!(max_consecutive(&h[1..], EnemyMove::Bellow) <= 1);
            assert!(max_consecutive(&h[1..], EnemyMove::Thrash) <= 2);
            for mv in [EnemyMove::Chomp, EnemyMove::Thrash, EnemyMove::Bellow] {
                assert!(h.contains(&mv), "seed {seed} never rolled {mv:?}");
            }
        }
    }

    #[test]
    fn wolf_distribution_is_roughly_25_30_45() {
        // Distribution check on raw rolls is awkward with constraints; instead
        // assert long-run frequencies sit in generous bands.
        let h = simulate(Species::GraveWolf, 3000, 99);
        let count = |mv| h.iter().filter(|m| **m == mv).count() as f64 / h.len() as f64;
        let (chomp, thrash, bellow) = (
            count(EnemyMove::Chomp),
            count(EnemyMove::Thrash),
            count(EnemyMove::Bellow),
        );
        assert!((0.15..=0.40).contains(&chomp), "chomp {chomp}");
        assert!((0.20..=0.45).contains(&thrash), "thrash {thrash}");
        assert!((0.30..=0.60).contains(&bellow), "bellow {bellow}");
    }

    #[test]
    fn rats_respect_repeat_rules() {
        for (species, special) in [
            (Species::BarrowRat, EnemyMove::Grow),
            (Species::FenRat, EnemyMove::Spittle),
        ] {
            for seed in 0..10 {
                let h = simulate(species, 300, seed);
                assert!(max_consecutive(&h, EnemyMove::Bite) <= 2);
                assert!(max_consecutive(&h, special) <= 1);
                assert!(h.contains(&EnemyMove::Bite));
                assert!(h.contains(&special));
            }
        }
    }

    #[test]
    fn troll_bellows_first_then_rushes_and_bashes() {
        for seed in 0..10 {
            let h = simulate(Species::ForestTroll, 300, seed);
            assert_eq!(h[0], EnemyMove::TrollBellow);
            assert!(!h[1..].contains(&EnemyMove::TrollBellow));
            assert!(max_consecutive(&h[1..], EnemyMove::Rush) <= 2);
            assert!(h[1..].contains(&EnemyMove::Rush));
            assert!(h[1..].contains(&EnemyMove::SkullBash));
        }
    }

    #[test]
    fn roll_move_is_deterministic_per_seed() {
        assert_eq!(
            simulate(Species::GraveWolf, 50, 1234),
            simulate(Species::GraveWolf, 50, 1234)
        );
    }

    #[test]
    fn new_species_data_matches_spec() {
        assert_eq!(Species::DraugrWarrior.name(), "Draugr Warrior");
        assert_eq!(Species::DraugrWarrior.hp_range(), (46, 50));
        assert_eq!(Species::MireCrawler.hp_range(), (22, 28));
        assert_eq!(Species::Hrafn.hp_range(), (30, 34));
        assert_eq!(Species::BarrowWight.hp_range(), (85, 90));
        assert_eq!(Species::DraugrWarlord.hp_range(), (86, 90));
        assert_eq!(Species::MoundJarl.name(), "The Mound Jarl");
        assert_eq!(Species::MoundJarl.hp_range(), (150, 150));
    }

    #[test]
    fn warrior_alternates_within_repeat_rules() {
        for seed in 0..10 {
            let h = simulate(Species::DraugrWarrior, 300, seed);
            assert!(max_consecutive(&h, EnemyMove::Stab) <= 2);
            assert!(max_consecutive(&h, EnemyMove::Rend) <= 2);
            assert!(h.contains(&EnemyMove::Stab) && h.contains(&EnemyMove::Rend));
        }
    }

    #[test]
    fn crawler_festers_first_then_mixes() {
        let h = simulate(Species::MireCrawler, 300, 3);
        assert_eq!(h[0], EnemyMove::Fester);
        assert!(max_consecutive(&h, EnemyMove::Fester) <= 1);
        assert!(h.contains(&EnemyMove::Bite));
    }

    #[test]
    fn wight_drains_first_then_mauls() {
        let h = simulate(Species::BarrowWight, 200, 5);
        assert_eq!(h[0], EnemyMove::SoulDrain);
        assert!(max_consecutive(&h, EnemyMove::SoulDrain) <= 1);
        assert!(h.contains(&EnemyMove::Maul));
    }

    #[test]
    fn warlord_chants_once_then_cleaves() {
        let h = simulate(Species::DraugrWarlord, 10, 1);
        assert_eq!(h[0], EnemyMove::WarChant);
        assert!(h[1..].iter().all(|m| *m == EnemyMove::Cleave));
    }

    #[test]
    fn jarl_rotation_is_fixed() {
        let h = simulate(Species::MoundJarl, 8, 1);
        use EnemyMove::*;
        assert_eq!(
            h,
            vec![
                DreadRoar,
                CrushingBlow,
                GraveCleave,
                Bulwark,
                DreadRoar,
                CrushingBlow,
                GraveCleave,
                Bulwark
            ]
        );
    }

    #[test]
    fn hrafn_uses_both_moves_within_repeat_rules() {
        for seed in 0..10 {
            let h = simulate(Species::Hrafn, 300, seed);
            assert!(max_consecutive(&h, EnemyMove::Peck) <= 2);
            assert!(max_consecutive(&h, EnemyMove::Screech) <= 2);
            assert!(h.contains(&EnemyMove::Peck) && h.contains(&EnemyMove::Screech));
        }
    }
}
