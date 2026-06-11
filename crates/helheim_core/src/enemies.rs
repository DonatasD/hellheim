#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    DraugrChanter, // StS Cultist
    GraveWolf,     // StS Jaw Worm
    BarrowRat,     // StS Red Louse
    FenRat,        // StS Green Louse
    ForestTroll,   // StS Gremlin Nob (elite)
}

impl Species {
    pub fn name(self) -> &'static str {
        match self {
            Species::DraugrChanter => "Draugr Chanter",
            Species::GraveWolf => "Grave Wolf",
            Species::BarrowRat => "Barrow Rat",
            Species::FenRat => "Fen Rat",
            Species::ForestTroll => "Forest Troll",
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
}
