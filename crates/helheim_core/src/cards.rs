/// Card behavior is data: the combat engine interprets `Effect` lists.
/// Numbers are Slay the Spire's (see spec); names are ours.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CardId {
    Hew,           // Strike
    RaiseShield,   // Defend
    SkullSplitter, // Bash
    WhirlingAxe,   // Cleave
    HaftStrike,    // Pommel Strike
    Unbowed,       // Shrug It Off
    ShieldCharge,  // Iron Wave
    TwinAxes,      // Twin Strike
    RisingFury,    // Anger
    SurgeOfRage,   // Flex
    Berserkergang, // Inflame
    ThorsWrath,    // Thunderclap
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardKind {
    Attack,
    Skill,
    Power,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targeting {
    SingleEnemy,
    AllEnemies,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    Damage(u32),
    DamageAll(u32),
    Block(u32),
    ApplyVulnerable(u32),
    ApplyVulnerableAll(u32),
    ApplyWeak(u32),
    ApplyWeakAll(u32),
    DamageEqualToBlock,
    Heal(u32),
    LoseHp(u32),
    GainEnergy(u32),
    GainStrength(i32),
    GainTempStrength(i32),
    Draw(u32),
    AddCopyToDiscard,
    GainRitual(u32),
    GainMetallicize(u32),
}

#[derive(Debug)]
pub struct CardSpec {
    pub id: CardId,
    pub name: &'static str,
    pub kind: CardKind,
    pub cost: u32,
    pub targeting: Targeting,
    pub effects: &'static [Effect],
    pub text: &'static str,
    pub exhausts: bool,
}

macro_rules! spec {
    ($id:ident, $name:literal, $kind:ident, $cost:literal, $tgt:ident, $fx:expr, $text:literal) => {
        CardSpec {
            id: CardId::$id,
            name: $name,
            kind: CardKind::$kind,
            cost: $cost,
            targeting: Targeting::$tgt,
            effects: $fx,
            text: $text,
            exhausts: false,
        }
    };
}

macro_rules! spec_x {
    ($id:ident, $name:literal, $kind:ident, $cost:literal, $tgt:ident, $fx:expr, $text:literal) => {
        CardSpec {
            id: CardId::$id,
            name: $name,
            kind: CardKind::$kind,
            cost: $cost,
            targeting: Targeting::$tgt,
            effects: $fx,
            text: $text,
            exhausts: true,
        }
    };
}

static SPECS: [CardSpec; 12] = [
    spec!(
        Hew,
        "Hew",
        Attack,
        1,
        SingleEnemy,
        &[Effect::Damage(6)],
        "Deal 6 damage."
    ),
    spec!(
        RaiseShield,
        "Raise Shield",
        Skill,
        1,
        None,
        &[Effect::Block(5)],
        "Gain 5 Block."
    ),
    spec!(
        SkullSplitter,
        "Skull-Splitter",
        Attack,
        2,
        SingleEnemy,
        &[Effect::Damage(8), Effect::ApplyVulnerable(2)],
        "Deal 8 damage. Apply 2 Vulnerable."
    ),
    spec!(
        WhirlingAxe,
        "Whirling Axe",
        Attack,
        1,
        AllEnemies,
        &[Effect::DamageAll(8)],
        "Deal 8 damage to ALL enemies."
    ),
    spec!(
        HaftStrike,
        "Haft Strike",
        Attack,
        1,
        SingleEnemy,
        &[Effect::Damage(9), Effect::Draw(1)],
        "Deal 9 damage. Draw 1 card."
    ),
    spec!(
        Unbowed,
        "Unbowed",
        Skill,
        1,
        None,
        &[Effect::Block(8), Effect::Draw(1)],
        "Gain 8 Block. Draw 1 card."
    ),
    spec!(
        ShieldCharge,
        "Shield Charge",
        Attack,
        1,
        SingleEnemy,
        &[Effect::Damage(5), Effect::Block(5)],
        "Deal 5 damage. Gain 5 Block."
    ),
    spec!(
        TwinAxes,
        "Twin Axes",
        Attack,
        1,
        SingleEnemy,
        &[Effect::Damage(5), Effect::Damage(5)],
        "Deal 5 damage twice."
    ),
    spec!(
        RisingFury,
        "Rising Fury",
        Attack,
        0,
        SingleEnemy,
        &[Effect::Damage(6), Effect::AddCopyToDiscard],
        "Deal 6 damage. Add a copy of this card to your discard pile."
    ),
    spec!(
        SurgeOfRage,
        "Surge of Rage",
        Skill,
        0,
        None,
        &[Effect::GainTempStrength(2)],
        "Gain 2 Strength. At the end of your turn, lose 2 Strength."
    ),
    spec!(
        Berserkergang,
        "Berserkergang",
        Power,
        1,
        None,
        &[Effect::GainStrength(2)],
        "Gain 2 Strength."
    ),
    spec!(
        ThorsWrath,
        "Thor's Wrath",
        Attack,
        1,
        AllEnemies,
        &[Effect::DamageAll(4), Effect::ApplyVulnerableAll(1)],
        "Deal 4 damage to ALL enemies. Apply 1 Vulnerable to ALL enemies."
    ),
];

impl CardId {
    /// # Panics
    /// Panics if the static table is missing an entry for this id (a bug).
    pub fn spec(self) -> &'static CardSpec {
        SPECS
            .iter()
            .find(|s| s.id == self)
            .expect("every CardId has a spec")
    }
}

pub fn starter_deck() -> Vec<CardId> {
    let mut deck = vec![CardId::Hew; 5];
    deck.extend(vec![CardId::RaiseShield; 4]);
    deck.push(CardId::SkullSplitter);
    deck
}

pub const REWARD_POOL: [CardId; 9] = [
    CardId::WhirlingAxe,
    CardId::HaftStrike,
    CardId::Unbowed,
    CardId::ShieldCharge,
    CardId::TwinAxes,
    CardId::RisingFury,
    CardId::SurgeOfRage,
    CardId::Berserkergang,
    CardId::ThorsWrath,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_match_the_design_table() {
        // Spot-check StS numbers per the spec's card table.
        let hew = CardId::Hew.spec();
        assert_eq!(hew.cost, 1);
        assert!(matches!(hew.kind, CardKind::Attack));
        assert!(matches!(hew.targeting, Targeting::SingleEnemy));
        assert_eq!(hew.effects, &[Effect::Damage(6)]);

        let bash = CardId::SkullSplitter.spec();
        assert_eq!(bash.cost, 2);
        assert_eq!(
            bash.effects,
            &[Effect::Damage(8), Effect::ApplyVulnerable(2)]
        );

        let cleave = CardId::WhirlingAxe.spec();
        assert!(matches!(cleave.targeting, Targeting::AllEnemies));
        assert_eq!(cleave.effects, &[Effect::DamageAll(8)]);

        let anger = CardId::RisingFury.spec();
        assert_eq!(anger.cost, 0);
        assert_eq!(
            anger.effects,
            &[Effect::Damage(6), Effect::AddCopyToDiscard]
        );

        let flex = CardId::SurgeOfRage.spec();
        assert_eq!(flex.cost, 0);
        assert_eq!(flex.effects, &[Effect::GainTempStrength(2)]);
        assert!(matches!(flex.targeting, Targeting::None));

        let inflame = CardId::Berserkergang.spec();
        assert!(matches!(inflame.kind, CardKind::Power));
        assert_eq!(inflame.effects, &[Effect::GainStrength(2)]);

        let thunderclap = CardId::ThorsWrath.spec();
        assert_eq!(
            thunderclap.effects,
            &[Effect::DamageAll(4), Effect::ApplyVulnerableAll(1)]
        );

        let twin = CardId::TwinAxes.spec();
        assert_eq!(twin.effects, &[Effect::Damage(5), Effect::Damage(5)]);

        let pommel = CardId::HaftStrike.spec();
        assert_eq!(pommel.effects, &[Effect::Damage(9), Effect::Draw(1)]);

        let shrug = CardId::Unbowed.spec();
        assert!(matches!(shrug.kind, CardKind::Skill));
        assert_eq!(shrug.effects, &[Effect::Block(8), Effect::Draw(1)]);

        let wave = CardId::ShieldCharge.spec();
        assert_eq!(wave.effects, &[Effect::Damage(5), Effect::Block(5)]);

        let defend = CardId::RaiseShield.spec();
        assert_eq!(defend.effects, &[Effect::Block(5)]);
    }

    #[test]
    fn starter_deck_is_5_hew_4_shield_1_bash() {
        let deck = starter_deck();
        assert_eq!(deck.len(), 10);
        assert_eq!(deck.iter().filter(|c| **c == CardId::Hew).count(), 5);
        assert_eq!(
            deck.iter().filter(|c| **c == CardId::RaiseShield).count(),
            4
        );
        assert_eq!(
            deck.iter().filter(|c| **c == CardId::SkullSplitter).count(),
            1
        );
    }

    #[test]
    fn existing_cards_do_not_exhaust() {
        assert!(!CardId::Hew.spec().exhausts);
        assert!(!CardId::Berserkergang.spec().exhausts);
    }

    #[test]
    fn reward_pool_is_the_9_non_starter_designs() {
        assert_eq!(REWARD_POOL.len(), 9);
        for starter in [CardId::Hew, CardId::RaiseShield, CardId::SkullSplitter] {
            assert!(!REWARD_POOL.contains(&starter));
        }
        let mut unique = REWARD_POOL.to_vec();
        unique.sort_unstable_by_key(|c| *c as usize);
        unique.dedup();
        assert_eq!(unique.len(), 9);
    }
}
