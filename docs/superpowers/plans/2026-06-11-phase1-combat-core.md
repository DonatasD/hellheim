# Helheim Phase 1 (Combat Core) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A playable Slay-the-Spire-style card combat game — pure-Rust rules engine + Bevy 0.18 shell — running a deterministic 3-fight gauntlet with card rewards, per the approved spec.

**Architecture:** Two-crate workspace: `helheim_core` (rules engine, zero Bevy deps, TDD'd) and `helheim` (Bevy shell). The shell drives the core with `Action`s; the core answers with ordered `CombatEvent`s; the shell replays events through a paced animation queue into a `DisplayState` that the UI renders.

**Tech Stack:** Rust stable (rustup), Bevy 0.18, rand 0.9 + rand_chacha 0.9.

---

## Context for the implementing engineer

- **Spec:** `docs/superpowers/specs/2026-06-10-helheim-design.md`. Read it first. Where this plan and the spec disagree on game numbers, the spec wins; where the spec defers to the Slay the Spire wiki, the wiki wins and tests encode the wiki's numbers.
- **TDD discipline:** every core task is red → green → commit. Run only the named test while iterating (`cargo test -p helheim_core <name>`), the full suite before each commit.
- **Bevy API drift warning:** shell code below is written for Bevy 0.18. If something doesn't compile, the cause is usually a renamed API (Bevy renames things every minor release — e.g. buffered `Event`/`EventReader` became `Message`/`MessageReader` in 0.17). Fix by checking the migration guides at <https://bevy.org/learn/migration-guides/> and docs.rs/bevy/0.18. The *structure* of the tasks stands even if names drift.
- **Build times:** the first `cargo build` after adding Bevy takes several minutes. Don't interpret that as a hang. Core-only test runs stay fast (`cargo test -p helheim_core`).
- **Commits:** small, after each green task step that says so. Message style: `feat(core): ...`, `feat(shell): ...`, `test: ...`, `chore: ...`.

## Type Reference (canonical — later tasks must match this exactly)

```rust
// rng.rs
pub struct RunRng(ChaCha8Rng);
impl RunRng {
    pub fn new(seed: u64) -> Self;
    pub fn range(&mut self, lo: u32, hi: u32) -> u32;   // inclusive both ends
    pub fn percent(&mut self) -> u32;                   // uniform 0..=99
    pub fn shuffle<T>(&mut self, xs: &mut [T]);
    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T;
}

// cards.rs
pub enum CardId { Hew, RaiseShield, SkullSplitter, WhirlingAxe, HaftStrike, Unbowed,
                  ShieldCharge, TwinAxes, RisingFury, SurgeOfRage, Berserkergang, ThorsWrath }
pub enum CardKind { Attack, Skill, Power }
pub enum Targeting { SingleEnemy, AllEnemies, None }
pub enum Effect { Damage(u32), DamageAll(u32), Block(u32), ApplyVulnerable(u32),
                  ApplyVulnerableAll(u32), GainStrength(i32), GainTempStrength(i32),
                  Draw(u32), AddCopyToDiscard }
pub struct CardSpec { pub id: CardId, pub name: &'static str, pub kind: CardKind, pub cost: u32,
                      pub targeting: Targeting, pub effects: &'static [Effect], pub text: &'static str }
impl CardId { pub fn spec(self) -> &'static CardSpec }
pub fn starter_deck() -> Vec<CardId>;
pub const REWARD_POOL: [CardId; 9];

// statuses.rs
pub struct Statuses { pub strength: i32, pub vulnerable: u32, pub weak: u32, pub ritual: u32,
                      pub ritual_fresh: bool, pub enrage: u32, pub curl_up: Option<u32>,
                      pub strength_down: u32 }
pub enum StatusKind { Strength, Vulnerable, Weak, Ritual, Enrage, CurlUp, StrengthDown }

// enemies.rs
pub enum Species { DraugrChanter, GraveWolf, BarrowRat, FenRat, ForestTroll }
impl Species { pub fn name(self) -> &'static str; pub fn hp_range(self) -> (u32, u32); }
pub enum EnemyMove { Chant, DarkStrike, Chomp, Thrash, Bellow, Bite, Grow, Spittle,
                     TrollBellow, Rush, SkullBash }
pub fn roll_move(species: Species, history: &[EnemyMove], rng: &mut RunRng) -> EnemyMove;

// combat.rs
pub struct Player { pub hp: u32, pub max_hp: u32, pub block: u32, pub energy: u32, pub statuses: Statuses }
pub struct Enemy  { pub species: Species, pub hp: u32, pub max_hp: u32, pub block: u32,
                    pub statuses: Statuses, pub bite_damage: u32, pub next_move: EnemyMove,
                    pub history: Vec<EnemyMove> }
pub struct CombatState { pub player: Player, pub enemies: Vec<Enemy>, pub draw: Vec<CardId>,
                         pub hand: Vec<CardId>, pub discard: Vec<CardId>, pub exhaust: Vec<CardId>,
                         pub turn: u32, pub over: Option<Outcome> }
pub enum Outcome { Victory, Defeat }
pub enum Action { PlayCard { hand_index: usize, target: Option<usize> }, EndTurn }
pub enum IllegalAction { CombatOver, NoSuchCard, NotEnoughEnergy, NeedsTarget, InvalidTarget }
pub enum TargetRef { Player, Enemy(usize) }
pub enum IntentKind { Attack { damage: u32, hits: u32 }, AttackDefend { damage: u32 }, Defend, Buff, Debuff }
pub enum CombatEvent {
    TurnStarted { turn: u32 },
    EnergySet { energy: u32 },
    CardDrawn { card: CardId },
    DeckShuffled,
    CardPlayed { card: CardId, hand_index: usize },
    CardAddedToDiscard { card: CardId },
    HandDiscarded,
    BlockReset { target: TargetRef },
    BlockGained { target: TargetRef, amount: u32 },
    DamageDealt { target: TargetRef, amount: u32, blocked: u32, hp_lost: u32 },
    StatusApplied { target: TargetRef, status: StatusKind, amount: i32 },
    StatusTicked { target: TargetRef, status: StatusKind, remaining: u32 },
    StatusExpired { target: TargetRef, status: StatusKind },
    EnemyMoved { index: usize, mv: EnemyMove },
    IntentSet { index: usize, intent: IntentKind },
    EnemyDied { index: usize },
    PlayerDied,
    Victory,
}
impl CombatState {
    pub fn new(rng: &mut RunRng, deck: &[CardId], hp: u32, max_hp: u32, species: &[Species])
        -> (CombatState, Vec<CombatEvent>);
    pub fn apply(&mut self, rng: &mut RunRng, action: Action)
        -> Result<Vec<CombatEvent>, IllegalAction>;
    pub fn intent_of(&self, index: usize) -> IntentKind;
}
pub fn attack_damage(base: u32, strength: i32, attacker_weak: bool, target_vulnerable: bool) -> u32;

// run.rs
pub enum Stage { Fight(u8), Reward { after_fight: u8, offer: [CardId; 3] }, Victory, Defeat }
pub struct RunStats { pub turns: u32, pub damage_dealt: u64, pub damage_taken: u64 }
pub enum RunError { NotInFight, NotInReward, BadIndex }
pub struct RunState { pub seed: u64, pub master_deck: Vec<CardId>, pub hp: u32, pub max_hp: u32,
                      pub stage: Stage, pub combat: Option<CombatState>, pub stats: RunStats /* rng private */ }
impl RunState {
    pub fn new(seed: u64) -> Self;                                    // stage Fight(1), combat None
    pub fn begin_fight(&mut self) -> Result<Vec<CombatEvent>, RunError>;
    pub fn apply(&mut self, action: Action) -> Result<Vec<CombatEvent>, IllegalAction>;
    pub fn choose_reward(&mut self, pick: Option<usize>) -> Result<(), RunError>;
}
```

Game-rule decisions locked here (tests encode them):

- Damage pipeline in integer math: `d = max(0, base + strength)`, then if attacker weak `d = d*3/4`, then if target vulnerable `d = d*3/2` (truncating division = floor).
- Durations (Vulnerable/Weak) decrement at the end of the afflicted creature's turn, including the turn they were applied.
- **Ritual does not fire at the end of the turn it was applied** (`ritual_fresh` flag) — Draugr Chanter hits 6, 9, 12, … exactly like the StS Cultist.
- Single-target damage effects fizzle (no event) if the chosen target died mid-card (Twin Axes' second hit after a kill).
- On Victory, remaining effects of the current card stop resolving.
- Enrage triggers after a Skill's effects resolve, and re-emits `IntentSet` for that enemy (attack numbers change mid-player-turn only via Enrage in Phase 1).
- Enemy indices are stable for a whole combat; dead enemies stay in `enemies` with `hp == 0`.
- Card order: `draw.pop()` is the top of the draw pile.

---

### Task 1: Install the Rust toolchain

The machine has no Rust (`cargo`/`rustc` not on PATH, no `~/.cargo`).

- [ ] **Step 1: Install rustup with the stable toolchain**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
```

- [ ] **Step 2: Load cargo into this shell and verify**

```bash
source "$HOME/.cargo/env"
cargo --version && rustc --version && cargo fmt --version && cargo clippy --version
```

Expected: four version lines (rustc 1.8x, stable). Note for all later steps: new shells need `source "$HOME/.cargo/env"` or `~/.cargo/bin` on PATH.

### Task 2: Scaffold the workspace and core crate

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/helheim_core/Cargo.toml`
- Create: `crates/helheim_core/src/lib.rs`

- [ ] **Step 1: Write the workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/helheim_core"]

# Fast dev iteration: light optimization for our code, full for dependencies.
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

- [ ] **Step 2: Write `crates/helheim_core/Cargo.toml`**

```toml
[package]
name = "helheim_core"
version = "0.1.0"
edition = "2021"

[dependencies]
rand = "0.9"
rand_chacha = "0.9"
```

- [ ] **Step 3: Write `crates/helheim_core/src/lib.rs`**

```rust
//! Helheim rules engine. No engine/UI dependencies — pure, deterministic, testable.
```

- [ ] **Step 4: Verify the workspace builds and the (empty) test suite runs**

Run: `cargo test -p helheim_core`
Expected: compiles, `running 0 tests`, `test result: ok`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/
git commit -m "chore: scaffold cargo workspace with helheim_core crate"
```

### Task 3: `rng.rs` — seeded deterministic RNG

**Files:**
- Create: `crates/helheim_core/src/rng.rs`
- Modify: `crates/helheim_core/src/lib.rs`

- [ ] **Step 1: Write the failing tests** — create `rng.rs` with only the test module, and register the module

`crates/helheim_core/src/rng.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = RunRng::new(42);
        let mut b = RunRng::new(42);
        let sa: Vec<u32> = (0..100).map(|_| a.range(0, 1000)).collect();
        let sb: Vec<u32> = (0..100).map(|_| b.range(0, 1000)).collect();
        assert_eq!(sa, sb);
    }

    #[test]
    fn different_seed_different_sequence() {
        let mut a = RunRng::new(1);
        let mut b = RunRng::new(2);
        let sa: Vec<u32> = (0..100).map(|_| a.range(0, 1000)).collect();
        let sb: Vec<u32> = (0..100).map(|_| b.range(0, 1000)).collect();
        assert_ne!(sa, sb);
    }

    #[test]
    fn range_is_inclusive_and_bounded() {
        let mut r = RunRng::new(7);
        let mut seen_lo = false;
        let mut seen_hi = false;
        for _ in 0..2000 {
            let v = r.range(3, 7);
            assert!((3..=7).contains(&v));
            seen_lo |= v == 3;
            seen_hi |= v == 7;
        }
        assert!(seen_lo && seen_hi);
    }

    #[test]
    fn percent_is_0_to_99() {
        let mut r = RunRng::new(7);
        for _ in 0..2000 {
            assert!(r.percent() <= 99);
        }
    }

    #[test]
    fn shuffle_is_deterministic_permutation() {
        let mut a = RunRng::new(9);
        let mut b = RunRng::new(9);
        let mut xs: Vec<u32> = (0..20).collect();
        let mut ys = xs.clone();
        a.shuffle(&mut xs);
        b.shuffle(&mut ys);
        assert_eq!(xs, ys);
        let mut sorted = xs.clone();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn pick_returns_an_element() {
        let mut r = RunRng::new(3);
        for _ in 0..100 {
            let v = r.pick(&[10u32, 20, 30]);
            assert!([10, 20, 30].contains(&v));
        }
    }
}
```

Add to `lib.rs`:

```rust
pub mod rng;
```

- [ ] **Step 2: Run tests to verify they fail to compile (RunRng undefined)**

Run: `cargo test -p helheim_core rng`
Expected: compile error: `cannot find type RunRng` (or similar).

- [ ] **Step 3: Implement `RunRng`** — prepend above the test module in `rng.rs`:

```rust
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// All game randomness flows through one seeded stream: same seed, same run.
pub struct RunRng(ChaCha8Rng);

impl RunRng {
    pub fn new(seed: u64) -> Self {
        Self(ChaCha8Rng::seed_from_u64(seed))
    }

    /// Uniform integer in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        self.0.random_range(lo..=hi)
    }

    /// Uniform 0..=99, for percentage rolls.
    pub fn percent(&mut self) -> u32 {
        self.0.random_range(0..100)
    }

    pub fn shuffle<T>(&mut self, xs: &mut [T]) {
        xs.shuffle(&mut self.0);
    }

    pub fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        *xs.choose(&mut self.0).expect("pick from empty slice")
    }
}
```

(rand 0.9 renamed `gen_range` → `random_range`; if the compiler complains about `choose`, it lives on `rand::seq::IndexedRandom` in rand 0.9 — adjust the import per the compiler's suggestion.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core rng`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): seeded deterministic RunRng wrapper"
```

### Task 4: `cards.rs` — card data table

**Files:**
- Create: `crates/helheim_core/src/cards.rs`
- Modify: `crates/helheim_core/src/lib.rs`

- [ ] **Step 1: Write the failing tests** — create `cards.rs` with only this test module; add `pub mod cards;` to `lib.rs`

```rust
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
        assert_eq!(bash.effects, &[Effect::Damage(8), Effect::ApplyVulnerable(2)]);

        let cleave = CardId::WhirlingAxe.spec();
        assert!(matches!(cleave.targeting, Targeting::AllEnemies));
        assert_eq!(cleave.effects, &[Effect::DamageAll(8)]);

        let anger = CardId::RisingFury.spec();
        assert_eq!(anger.cost, 0);
        assert_eq!(anger.effects, &[Effect::Damage(6), Effect::AddCopyToDiscard]);

        let flex = CardId::SurgeOfRage.spec();
        assert_eq!(flex.cost, 0);
        assert_eq!(flex.effects, &[Effect::GainTempStrength(2)]);
        assert!(matches!(flex.targeting, Targeting::None));

        let inflame = CardId::Berserkergang.spec();
        assert!(matches!(inflame.kind, CardKind::Power));
        assert_eq!(inflame.effects, &[Effect::GainStrength(2)]);

        let thunderclap = CardId::ThorsWrath.spec();
        assert_eq!(thunderclap.effects, &[Effect::DamageAll(4), Effect::ApplyVulnerableAll(1)]);

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
        assert_eq!(deck.iter().filter(|c| **c == CardId::RaiseShield).count(), 4);
        assert_eq!(deck.iter().filter(|c| **c == CardId::SkullSplitter).count(), 1);
    }

    #[test]
    fn reward_pool_is_the_9_non_starter_designs() {
        assert_eq!(REWARD_POOL.len(), 9);
        for starter in [CardId::Hew, CardId::RaiseShield, CardId::SkullSplitter] {
            assert!(!REWARD_POOL.contains(&starter));
        }
        let mut unique = REWARD_POOL.to_vec();
        unique.dedup();
        assert_eq!(unique.len(), 9);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core cards`
Expected: compile error (`CardId` undefined).

- [ ] **Step 3: Implement the card table** — prepend to `cards.rs`:

```rust
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
    GainStrength(i32),
    GainTempStrength(i32),
    Draw(u32),
    AddCopyToDiscard,
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
        }
    };
}

static SPECS: [CardSpec; 12] = [
    spec!(Hew, "Hew", Attack, 1, SingleEnemy, &[Effect::Damage(6)], "Deal 6 damage."),
    spec!(RaiseShield, "Raise Shield", Skill, 1, None, &[Effect::Block(5)], "Gain 5 Block."),
    spec!(SkullSplitter, "Skull-Splitter", Attack, 2, SingleEnemy,
          &[Effect::Damage(8), Effect::ApplyVulnerable(2)], "Deal 8 damage. Apply 2 Vulnerable."),
    spec!(WhirlingAxe, "Whirling Axe", Attack, 1, AllEnemies,
          &[Effect::DamageAll(8)], "Deal 8 damage to ALL enemies."),
    spec!(HaftStrike, "Haft Strike", Attack, 1, SingleEnemy,
          &[Effect::Damage(9), Effect::Draw(1)], "Deal 9 damage. Draw 1 card."),
    spec!(Unbowed, "Unbowed", Skill, 1, None,
          &[Effect::Block(8), Effect::Draw(1)], "Gain 8 Block. Draw 1 card."),
    spec!(ShieldCharge, "Shield Charge", Attack, 1, SingleEnemy,
          &[Effect::Damage(5), Effect::Block(5)], "Deal 5 damage. Gain 5 Block."),
    spec!(TwinAxes, "Twin Axes", Attack, 1, SingleEnemy,
          &[Effect::Damage(5), Effect::Damage(5)], "Deal 5 damage twice."),
    spec!(RisingFury, "Rising Fury", Attack, 0, SingleEnemy,
          &[Effect::Damage(6), Effect::AddCopyToDiscard],
          "Deal 6 damage. Add a copy of this card to your discard pile."),
    spec!(SurgeOfRage, "Surge of Rage", Skill, 0, None,
          &[Effect::GainTempStrength(2)], "Gain 2 Strength. At the end of your turn, lose 2 Strength."),
    spec!(Berserkergang, "Berserkergang", Power, 1, None,
          &[Effect::GainStrength(2)], "Gain 2 Strength."),
    spec!(ThorsWrath, "Thor's Wrath", Attack, 1, AllEnemies,
          &[Effect::DamageAll(4), Effect::ApplyVulnerableAll(1)],
          "Deal 4 damage to ALL enemies. Apply 1 Vulnerable to ALL enemies."),
];

impl CardId {
    pub fn spec(self) -> &'static CardSpec {
        SPECS.iter().find(|s| s.id == self).expect("every CardId has a spec")
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core cards`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): card data table with the 12 Phase 1 designs"
```

### Task 5: `statuses.rs` — status container and duration ticking

**Files:**
- Create: `crates/helheim_core/src/statuses.rs`
- Modify: `crates/helheim_core/src/lib.rs` (add `pub mod statuses;`)

- [ ] **Step 1: Write the failing tests** — create `statuses.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_zero() {
        let s = Statuses::default();
        assert_eq!(s.strength, 0);
        assert_eq!(s.vulnerable, 0);
        assert_eq!(s.weak, 0);
        assert_eq!(s.ritual, 0);
        assert!(!s.ritual_fresh);
        assert_eq!(s.enrage, 0);
        assert_eq!(s.curl_up, None);
        assert_eq!(s.strength_down, 0);
    }

    #[test]
    fn durations_stack_additively() {
        let mut s = Statuses::default();
        s.vulnerable += 2;
        s.vulnerable += 1;
        assert_eq!(s.vulnerable, 3);
    }

    #[test]
    fn tick_durations_decrements_and_reports() {
        let mut s = Statuses {
            vulnerable: 2,
            weak: 1,
            ..Default::default()
        };
        let ticked = s.tick_durations();
        assert_eq!(s.vulnerable, 1);
        assert_eq!(s.weak, 0);
        assert_eq!(ticked, vec![(StatusKind::Vulnerable, 1), (StatusKind::Weak, 0)]);

        let ticked = s.tick_durations();
        assert_eq!(s.vulnerable, 0);
        assert_eq!(ticked, vec![(StatusKind::Vulnerable, 0)]);

        assert_eq!(s.tick_durations(), vec![]);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core statuses`
Expected: compile error (`Statuses` undefined).

- [ ] **Step 3: Implement** — prepend to `statuses.rs`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusKind {
    Strength,
    Vulnerable,
    Weak,
    Ritual,
    Enrage,
    CurlUp,
    StrengthDown,
}

/// One creature's statuses. Typed fields (not a map): the Phase 1 set is
/// closed, and exhaustive struct access keeps rule code obvious.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Statuses {
    /// Adds to each attack hit. Can go negative.
    pub strength: i32,
    /// Turns of taking ×1.5 attack damage.
    pub vulnerable: u32,
    /// Turns of dealing ×0.75 attack damage.
    pub weak: u32,
    /// Gains this much Strength at end of own turn…
    pub ritual: u32,
    /// …except the turn Ritual was applied (StS Cultist hits 6, 9, 12…).
    pub ritual_fresh: bool,
    /// Gains this much Strength whenever the *player* plays a Skill.
    pub enrage: u32,
    /// One-shot: gain this much Block the first time an attack deals ≥1 damage.
    pub curl_up: Option<u32>,
    /// Loses this much Strength at end of own turn, then clears (Surge of Rage).
    pub strength_down: u32,
}

impl Statuses {
    /// End-of-own-turn duration tick. Returns (kind, remaining) per decrement,
    /// in fixed order, so the combat engine can emit events.
    pub fn tick_durations(&mut self) -> Vec<(StatusKind, u32)> {
        let mut out = Vec::new();
        if self.vulnerable > 0 {
            self.vulnerable -= 1;
            out.push((StatusKind::Vulnerable, self.vulnerable));
        }
        if self.weak > 0 {
            self.weak -= 1;
            out.push((StatusKind::Weak, self.weak));
        }
        out
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core statuses`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): status container with duration ticking"
```

### Task 6: Damage pipeline — `combat.rs` foundations

**Files:**
- Create: `crates/helheim_core/src/combat.rs`
- Modify: `crates/helheim_core/src/lib.rs` (add `pub mod combat;`)

- [ ] **Step 1: Write the failing tests** — create `combat.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_damage_passes_through() {
        assert_eq!(attack_damage(6, 0, false, false), 6);
    }

    #[test]
    fn strength_adds_before_multipliers() {
        assert_eq!(attack_damage(6, 3, false, false), 9);
    }

    #[test]
    fn negative_strength_clamps_at_zero() {
        assert_eq!(attack_damage(2, -5, false, false), 0);
    }

    #[test]
    fn weak_is_three_quarters_floored() {
        assert_eq!(attack_damage(6, 0, true, false), 4); // floor(4.5)
        assert_eq!(attack_damage(7, 0, true, false), 5); // floor(5.25)
        assert_eq!(attack_damage(8, 0, true, false), 6); // exact
    }

    #[test]
    fn vulnerable_is_one_and_a_half_floored() {
        assert_eq!(attack_damage(6, 0, false, true), 9); // exact
        assert_eq!(attack_damage(5, 0, false, true), 7); // floor(7.5)
    }

    #[test]
    fn pipeline_order_is_strength_then_weak_then_vulnerable() {
        // (6+2)=8 → weak: 8*3/4=6 → vuln: 6*3/2=9.
        // Wrong order (vuln before weak) gives floor(12*0.75)=9 here, so use
        // a case that distinguishes: (6+1)=7 → weak 5 → vuln 7.
        // Vuln-first would be 7→10→7 (10*3/4=7)… also 7. Use base 9:
        // 9 → weak 6 → vuln 9.  Vuln first: 9→13 (13.5 floored)→9 (9.75 floored).
        // Distinguishing case: base 10 → weak 7 → vuln 10.
        // Vuln first: 10→15→11. Assert 10.
        assert_eq!(attack_damage(10, 0, true, true), 10);
    }

    #[test]
    fn soak_consumes_block_before_hp() {
        let mut block = 5;
        let mut hp = 80;
        let out = soak(&mut block, &mut hp, 8);
        assert_eq!((block, hp), (0, 77));
        assert_eq!((out.blocked, out.hp_lost), (5, 3));
    }

    #[test]
    fn soak_leaves_leftover_block() {
        let mut block = 10;
        let mut hp = 80;
        let out = soak(&mut block, &mut hp, 4);
        assert_eq!((block, hp), (6, 80));
        assert_eq!((out.blocked, out.hp_lost), (4, 0));
    }

    #[test]
    fn soak_does_not_underflow_hp() {
        let mut block = 0;
        let mut hp = 3;
        let out = soak(&mut block, &mut hp, 99);
        assert_eq!(hp, 0);
        assert_eq!(out.hp_lost, 3);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core combat`
Expected: compile error (`attack_damage` undefined).

- [ ] **Step 3: Implement the pipeline** — prepend to `combat.rs`:

```rust
/// StS damage pipeline, integer math (truncating division == floor here):
/// strength adds to base, Weak multiplies ×0.75, Vulnerable ×1.5.
pub fn attack_damage(base: u32, strength: i32, attacker_weak: bool, target_vulnerable: bool) -> u32 {
    let mut d = (base as i32 + strength).max(0) as u32;
    if attacker_weak {
        d = d * 3 / 4;
    }
    if target_vulnerable {
        d = d * 3 / 2;
    }
    d
}

pub struct DamageOutcome {
    pub blocked: u32,
    pub hp_lost: u32,
}

/// Apply damage to block first, overflow to HP. Mutates in place.
pub fn soak(block: &mut u32, hp: &mut u32, amount: u32) -> DamageOutcome {
    let blocked = amount.min(*block);
    *block -= blocked;
    let hp_lost = (amount - blocked).min(*hp);
    *hp -= hp_lost;
    DamageOutcome { blocked, hp_lost }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core combat`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): StS-faithful damage pipeline and block soak"
```

### Task 7: `enemies.rs` — species data and moves

**Files:**
- Create: `crates/helheim_core/src/enemies.rs`
- Modify: `crates/helheim_core/src/lib.rs` (add `pub mod enemies;`)

- [ ] **Step 1: Write the failing tests** — create `enemies.rs` with this test module:

```rust
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
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core enemies`
Expected: compile error (`Species` undefined).

- [ ] **Step 3: Implement species and moves** — prepend to `enemies.rs`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core enemies`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): enemy species data and move definitions"
```

### Task 8: Enemy AI — `roll_move` with no-repeat rules

**Files:**
- Modify: `crates/helheim_core/src/enemies.rs`

The AI contract (spec numbers; StS wiki is authoritative if they conflict — if you
correct a number from the wiki, update spec + tests together in the same commit):

- Draugr Chanter: turn 1 `Chant`, every later turn `DarkStrike`. No randomness.
- Grave Wolf: turn 1 always `Chomp`. Later: 25% `Chomp` / 30% `Thrash` / 45% `Bellow`;
  `Chomp` and `Bellow` never twice in a row, `Thrash` at most twice in a row.
- Rats: 75% `Bite` / 25% other (`Grow` for Barrow, `Spittle` for Fen);
  `Bite` at most twice in a row, the other move never twice in a row.
- Forest Troll: turn 1 `TrollBellow`. Later: 33% `SkullBash` / 67% `Rush`;
  `Rush` at most twice in a row.

"Turn 1" means `history.is_empty()`. Constraint handling: roll, and if the roll
violates a no-repeat rule, re-roll until it doesn't (StS does the same; with
these rule sets a legal move always exists, so the loop terminates).

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `enemies.rs`:

```rust
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
            assert!(max_consecutive(&h[1..], EnemyMove::Chomp) <= 1);
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
        let (chomp, thrash, bellow) =
            (count(EnemyMove::Chomp), count(EnemyMove::Thrash), count(EnemyMove::Bellow));
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
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core enemies`
Expected: compile error (`roll_move` undefined).

- [ ] **Step 3: Implement `roll_move`** — add to `enemies.rs` (above tests):

```rust
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
            if first_turn { EnemyMove::Chant } else { EnemyMove::DarkStrike }
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
                let candidate = if rng.percent() < 75 { EnemyMove::Bite } else { special };
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
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core enemies`
Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): StS-faithful enemy move AI with no-repeat rules"
```
### Task 9: `CombatState` — structure, combat start, intents

**Files:**
- Modify: `crates/helheim_core/src/combat.rs`

- [ ] **Step 1: Write the failing tests** — append inside `mod tests` in `combat.rs`:

```rust
    use crate::cards::{starter_deck, CardId};
    use crate::enemies::{EnemyMove, Species};
    use crate::rng::RunRng;
    use crate::statuses::Statuses;

    // ---- hand-built fixtures (no RNG dependence) ----

    pub fn enemy(species: Species, hp: u32) -> Enemy {
        Enemy {
            species,
            hp,
            max_hp: hp,
            block: 0,
            statuses: Statuses::default(),
            bite_damage: 6,
            next_move: EnemyMove::DarkStrike,
            history: vec![EnemyMove::Chant], // non-empty: not "turn 1" for AI rolls
        }
    }

    pub fn combat_vs(enemies: Vec<Enemy>, hand: Vec<CardId>) -> CombatState {
        CombatState {
            player: Player { hp: 80, max_hp: 80, block: 0, energy: 3, statuses: Statuses::default() },
            enemies,
            draw: vec![],
            hand,
            discard: vec![],
            exhaust: vec![],
            turn: 1,
            over: None,
        }
    }

    fn count_drawn(events: &[CombatEvent]) -> usize {
        events.iter().filter(|e| matches!(e, CombatEvent::CardDrawn { .. })).count()
    }

    #[test]
    fn new_combat_opens_with_5_cards_3_energy_and_intents() {
        let mut rng = RunRng::new(11);
        let (c, events) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        assert_eq!(c.hand.len(), 5);
        assert_eq!(c.draw.len(), 5);
        assert_eq!(c.player.energy, 3);
        assert_eq!(c.turn, 1);
        assert!(c.over.is_none());
        assert_eq!(count_drawn(&events), 5);
        assert!(events.iter().any(|e| matches!(e, CombatEvent::TurnStarted { turn: 1 })));
        assert!(events.iter().any(|e| matches!(e, CombatEvent::IntentSet { index: 0, .. })));
        // Wolf always opens with Chomp: base 11.
        assert_eq!(c.enemies[0].next_move, EnemyMove::Chomp);
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 11, hits: 1 }));
    }

    #[test]
    fn new_combat_rolls_hp_in_species_range() {
        let mut rng = RunRng::new(5);
        let (c, _) = CombatState::new(
            &mut rng, &starter_deck(), 80, 80,
            &[Species::BarrowRat, Species::FenRat],
        );
        let (lo0, hi0) = Species::BarrowRat.hp_range();
        assert!((lo0..=hi0).contains(&c.enemies[0].hp));
        // Rats spawn with Curl Up 3–7 and bite 5–7.
        for e in &c.enemies {
            let curl = e.statuses.curl_up.expect("rats spawn with Curl Up");
            assert!((3..=7).contains(&curl));
            assert!((5..=7).contains(&e.bite_damage));
        }
    }

    #[test]
    fn new_combat_is_deterministic_per_seed() {
        let mk = || {
            let mut rng = RunRng::new(77);
            CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::DraugrChanter])
        };
        let (a, ea) = mk();
        let (b, eb) = mk();
        assert_eq!(a.hand, b.hand);
        assert_eq!(a.enemies[0].hp, b.enemies[0].hp);
        assert_eq!(format!("{ea:?}"), format!("{eb:?}"));
    }

    #[test]
    fn intent_reflects_enemy_strength_and_player_vulnerable() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 6, hits: 1 }));
        c.enemies[0].statuses.strength = 3;
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 9, hits: 1 }));
        c.player.statuses.vulnerable = 1;
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 13, hits: 1 })); // floor(9*1.5)
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core combat`
Expected: compile error (`CombatState` undefined).

- [ ] **Step 3: Implement the types, `new`, `intent_of`, and the draw helper** — add to `combat.rs` after the damage functions:

```rust
use crate::cards::CardId;
use crate::enemies::{roll_move, EnemyMove, Species};
use crate::rng::RunRng;
use crate::statuses::{StatusKind, Statuses};

pub const ENERGY_PER_TURN: u32 = 3;
pub const DRAW_PER_TURN: usize = 5;
pub const HAND_LIMIT: usize = 10;

#[derive(Clone, Debug)]
pub struct Player {
    pub hp: u32,
    pub max_hp: u32,
    pub block: u32,
    pub energy: u32,
    pub statuses: Statuses,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub species: Species,
    pub hp: u32,
    pub max_hp: u32,
    pub block: u32,
    pub statuses: Statuses,
    /// Rolled 5–7 at spawn for rats; 0 for species without a Bite.
    pub bite_damage: u32,
    pub next_move: EnemyMove,
    pub history: Vec<EnemyMove>,
}

impl Enemy {
    pub fn alive(&self) -> bool {
        self.hp > 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome { Victory, Defeat }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayCard { hand_index: usize, target: Option<usize> },
    EndTurn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IllegalAction { CombatOver, NoSuchCard, NotEnoughEnergy, NeedsTarget, InvalidTarget }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetRef { Player, Enemy(usize) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntentKind {
    Attack { damage: u32, hits: u32 },
    AttackDefend { damage: u32 },
    Defend,
    Buff,
    Debuff,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CombatEvent {
    TurnStarted { turn: u32 },
    EnergySet { energy: u32 },
    CardDrawn { card: CardId },
    DeckShuffled,
    CardPlayed { card: CardId, hand_index: usize },
    CardAddedToDiscard { card: CardId },
    HandDiscarded,
    BlockReset { target: TargetRef },
    BlockGained { target: TargetRef, amount: u32 },
    DamageDealt { target: TargetRef, amount: u32, blocked: u32, hp_lost: u32 },
    StatusApplied { target: TargetRef, status: StatusKind, amount: i32 },
    StatusTicked { target: TargetRef, status: StatusKind, remaining: u32 },
    StatusExpired { target: TargetRef, status: StatusKind },
    EnemyMoved { index: usize, mv: EnemyMove },
    IntentSet { index: usize, intent: IntentKind },
    EnemyDied { index: usize },
    PlayerDied,
    Victory,
}

#[derive(Clone, Debug)]
pub struct CombatState {
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub draw: Vec<CardId>,
    pub hand: Vec<CardId>,
    pub discard: Vec<CardId>,
    pub exhaust: Vec<CardId>,
    pub turn: u32,
    pub over: Option<Outcome>,
}

impl CombatState {
    /// Begin a fight. RNG draw order per enemy: HP, then bite (rats), then
    /// Curl Up (rats) — keep this order stable, determinism tests depend on it.
    pub fn new(
        rng: &mut RunRng,
        deck: &[CardId],
        hp: u32,
        max_hp: u32,
        species: &[Species],
    ) -> (CombatState, Vec<CombatEvent>) {
        let mut events = Vec::new();
        let mut draw = deck.to_vec();
        rng.shuffle(&mut draw);

        let enemies: Vec<Enemy> = species
            .iter()
            .map(|&sp| {
                let (lo, hi) = sp.hp_range();
                let hp = rng.range(lo, hi);
                let is_rat = matches!(sp, Species::BarrowRat | Species::FenRat);
                let bite_damage = if is_rat { rng.range(5, 7) } else { 0 };
                let curl_up = is_rat.then(|| rng.range(3, 7));
                Enemy {
                    species: sp,
                    hp,
                    max_hp: hp,
                    block: 0,
                    statuses: Statuses { curl_up, ..Default::default() },
                    bite_damage,
                    next_move: EnemyMove::Chant, // placeholder, rolled below
                    history: Vec::new(),
                }
            })
            .collect();

        let mut state = CombatState {
            player: Player { hp, max_hp, block: 0, energy: ENERGY_PER_TURN, statuses: Statuses::default() },
            enemies,
            draw,
            hand: Vec::new(),
            discard: Vec::new(),
            exhaust: Vec::new(),
            turn: 1,
            over: None,
        };

        for i in 0..state.enemies.len() {
            state.enemies[i].next_move =
                roll_move(state.enemies[i].species, &state.enemies[i].history, rng);
            events.push(CombatEvent::IntentSet { index: i, intent: state.intent_of(i) });
        }

        events.push(CombatEvent::TurnStarted { turn: 1 });
        events.push(CombatEvent::EnergySet { energy: ENERGY_PER_TURN });
        for _ in 0..DRAW_PER_TURN {
            state.draw_one(rng, &mut events);
        }
        (state, events)
    }

    /// What the enemy's next move will do, with current modifiers baked in.
    pub fn intent_of(&self, index: usize) -> IntentKind {
        let e = &self.enemies[index];
        let atk = |base: u32| {
            attack_damage(base, e.statuses.strength, e.statuses.weak > 0,
                          self.player.statuses.vulnerable > 0)
        };
        match e.next_move {
            EnemyMove::DarkStrike => IntentKind::Attack { damage: atk(6), hits: 1 },
            EnemyMove::Chomp => IntentKind::Attack { damage: atk(11), hits: 1 },
            EnemyMove::Rush => IntentKind::Attack { damage: atk(14), hits: 1 },
            EnemyMove::SkullBash => IntentKind::Attack { damage: atk(6), hits: 1 },
            EnemyMove::Bite => IntentKind::Attack { damage: atk(e.bite_damage), hits: 1 },
            EnemyMove::Thrash => IntentKind::AttackDefend { damage: atk(7) },
            EnemyMove::Chant | EnemyMove::Bellow | EnemyMove::Grow | EnemyMove::TrollBellow => {
                IntentKind::Buff
            }
            EnemyMove::Spittle => IntentKind::Debuff,
        }
    }

    /// Draw one card. Reshuffles discard into draw when needed; forfeits the
    /// draw silently at the hand limit or when both piles are empty.
    fn draw_one(&mut self, rng: &mut RunRng, events: &mut Vec<CombatEvent>) {
        if self.hand.len() >= HAND_LIMIT {
            return;
        }
        if self.draw.is_empty() {
            if self.discard.is_empty() {
                return;
            }
            self.draw.append(&mut self.discard);
            rng.shuffle(&mut self.draw);
            events.push(CombatEvent::DeckShuffled);
        }
        let card = self.draw.pop().expect("draw pile refilled above");
        self.hand.push(card);
        events.push(CombatEvent::CardDrawn { card });
    }
}
```

Note: `SkullBash` displays as a plain Attack intent (StS shows attack+debuff;
that nuance is dropped in Phase 1 — the number is what matters).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core combat`
Expected: all pass (13 so far in this module).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): CombatState with combat start, intents, and drawing"
```

### Task 10: PlayCard — validation, energy, basic damage and block

**Files:**
- Modify: `crates/helheim_core/src/combat.rs`

- [ ] **Step 1: Write the failing tests** — append inside `mod tests`:

```rust
    fn play(c: &mut CombatState, i: usize, t: Option<usize>) -> Result<Vec<CombatEvent>, IllegalAction> {
        let mut rng = RunRng::new(0);
        c.apply(&mut rng, Action::PlayCard { hand_index: i, target: t })
    }

    #[test]
    fn hew_deals_6_costs_1_and_goes_to_discard() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::Hew]);
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 44);
        assert_eq!(c.player.energy, 2);
        assert!(c.hand.is_empty());
        assert_eq!(c.discard, vec![CardId::Hew]);
        assert!(events.contains(&CombatEvent::CardPlayed { card: CardId::Hew, hand_index: 0 }));
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0), amount: 6, blocked: 0, hp_lost: 6
        }));
        assert!(events.contains(&CombatEvent::EnergySet { energy: 2 }));
    }

    #[test]
    fn raise_shield_gains_5_block() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::RaiseShield]);
        let events = play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.block, 5);
        assert!(events.contains(&CombatEvent::BlockGained { target: TargetRef::Player, amount: 5 }));
    }

    #[test]
    fn attack_consumes_enemy_block_first() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::Hew]);
        c.enemies[0].block = 4;
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 48);
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0), amount: 6, blocked: 4, hp_lost: 2
        }));
    }

    #[test]
    fn not_enough_energy_is_rejected() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::SkullSplitter]);
        c.player.energy = 1; // Skull-Splitter costs 2
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::NotEnoughEnergy));
        assert_eq!(c.hand.len(), 1); // nothing changed
        assert_eq!(c.player.energy, 1);
    }

    #[test]
    fn bad_hand_index_is_rejected() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::NoSuchCard));
    }

    #[test]
    fn single_target_card_with_two_living_enemies_needs_a_target() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 12), enemy(Species::FenRat, 12)],
            vec![CardId::Hew],
        );
        assert_eq!(play(&mut c, 0, None), Err(IllegalAction::NeedsTarget));
    }

    #[test]
    fn single_living_enemy_is_auto_targeted() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 12), enemy(Species::FenRat, 12)],
            vec![CardId::Hew],
        );
        c.enemies[0].hp = 0; // only the Fen Rat lives
        play(&mut c, 0, None).unwrap();
        assert_eq!(c.enemies[1].hp, 6);
    }

    #[test]
    fn dead_or_out_of_range_target_is_rejected() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 12), enemy(Species::FenRat, 12)],
            vec![CardId::Hew, CardId::Hew],
        );
        c.enemies[0].hp = 0;
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::InvalidTarget));
        assert_eq!(play(&mut c, 0, Some(9)), Err(IllegalAction::InvalidTarget));
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core combat`
Expected: compile error (`apply` not defined).

- [ ] **Step 3: Implement `apply`/`play_card` and the player-side attack helper** — add inside `impl CombatState`:

```rust
    pub fn apply(
        &mut self,
        rng: &mut RunRng,
        action: Action,
    ) -> Result<Vec<CombatEvent>, IllegalAction> {
        if self.over.is_some() {
            return Err(IllegalAction::CombatOver);
        }
        match action {
            Action::PlayCard { hand_index, target } => self.play_card(rng, hand_index, target),
            Action::EndTurn => Ok(self.end_turn(rng)),
        }
    }

    fn living(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.enemies.len()).filter(|&i| self.enemies[i].alive())
    }

    fn play_card(
        &mut self,
        rng: &mut RunRng,
        hand_index: usize,
        target: Option<usize>,
    ) -> Result<Vec<CombatEvent>, IllegalAction> {
        use crate::cards::{CardKind, Targeting};

        let card = *self.hand.get(hand_index).ok_or(IllegalAction::NoSuchCard)?;
        let spec = card.spec();
        if self.player.energy < spec.cost {
            return Err(IllegalAction::NotEnoughEnergy);
        }
        let target_idx = match spec.targeting {
            Targeting::SingleEnemy => match target {
                Some(t) => {
                    if self.enemies.get(t).map(Enemy::alive).unwrap_or(false) {
                        Some(t)
                    } else {
                        return Err(IllegalAction::InvalidTarget);
                    }
                }
                Option::None => {
                    let living: Vec<usize> = self.living().collect();
                    if living.len() == 1 {
                        Some(living[0]) // auto-target the only enemy
                    } else {
                        return Err(IllegalAction::NeedsTarget);
                    }
                }
            },
            Targeting::AllEnemies | Targeting::None => Option::None,
        };

        let mut events = Vec::new();
        self.hand.remove(hand_index);
        self.player.energy -= spec.cost;
        events.push(CombatEvent::CardPlayed { card, hand_index });
        events.push(CombatEvent::EnergySet { energy: self.player.energy });

        for effect in spec.effects {
            if self.over.is_some() {
                break; // victory stops remaining effects
            }
            self.run_effect(rng, *effect, card, target_idx, &mut events);
        }

        if spec.kind == CardKind::Skill && self.over.is_none() {
            self.trigger_enrage(&mut events);
        }
        if spec.kind != CardKind::Power {
            self.discard.push(card); // powers are consumed
        }
        Ok(events)
    }

    /// Player attacks one enemy: pipeline, soak, Curl Up, death, victory.
    fn attack_enemy(&mut self, i: usize, base: u32, events: &mut Vec<CombatEvent>) {
        let dmg = attack_damage(
            base,
            self.player.statuses.strength,
            self.player.statuses.weak > 0,
            self.enemies[i].statuses.vulnerable > 0,
        );
        let e = &mut self.enemies[i];
        let out = soak(&mut e.block, &mut e.hp, dmg);
        events.push(CombatEvent::DamageDealt {
            target: TargetRef::Enemy(i),
            amount: dmg,
            blocked: out.blocked,
            hp_lost: out.hp_lost,
        });
        if dmg >= 1 && e.hp > 0 {
            if let Some(curl) = e.statuses.curl_up.take() {
                e.block += curl;
                events.push(CombatEvent::BlockGained { target: TargetRef::Enemy(i), amount: curl });
                events.push(CombatEvent::StatusExpired { target: TargetRef::Enemy(i), status: StatusKind::CurlUp });
            }
        }
        if e.hp == 0 {
            events.push(CombatEvent::EnemyDied { index: i });
            if self.living().next().is_none() {
                self.over = Some(Outcome::Victory);
                events.push(CombatEvent::Victory);
            }
        }
    }

    fn trigger_enrage(&mut self, events: &mut Vec<CombatEvent>) {
        for i in 0..self.enemies.len() {
            let enrage = self.enemies[i].statuses.enrage;
            if self.enemies[i].alive() && enrage > 0 {
                self.enemies[i].statuses.strength += enrage as i32;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: enrage as i32,
                });
                events.push(CombatEvent::IntentSet { index: i, intent: self.intent_of(i) });
            }
        }
    }
```

And the first slice of `run_effect` (the rest lands in Tasks 11–12) — add inside `impl CombatState`:

```rust
    fn run_effect(
        &mut self,
        rng: &mut RunRng,
        effect: crate::cards::Effect,
        card: CardId,
        target: Option<usize>,
        events: &mut Vec<CombatEvent>,
    ) {
        use crate::cards::Effect;
        match effect {
            Effect::Damage(base) => {
                if let Some(t) = target {
                    if self.enemies[t].alive() {
                        self.attack_enemy(t, base, events);
                    } // dead target: the hit fizzles, no event
                }
            }
            Effect::Block(n) => {
                self.player.block += n;
                events.push(CombatEvent::BlockGained { target: TargetRef::Player, amount: n });
            }
            _ => todo!("Tasks 11–12"),
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core combat`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): play-card validation, energy, basic attack and block"
```

### Task 11: Card effects — draw, AoE, multi-hit, Curl Up, Vulnerable

**Files:**
- Modify: `crates/helheim_core/src/combat.rs`

- [ ] **Step 1: Write the failing tests** — append inside `mod tests`:

```rust
    #[test]
    fn haft_strike_draws_a_card() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::HaftStrike]);
        c.draw = vec![CardId::Hew];
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.hand, vec![CardId::Hew]);
        assert!(events.contains(&CombatEvent::CardDrawn { card: CardId::Hew }));
    }

    #[test]
    fn drawing_from_empty_pile_reshuffles_discard() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::Unbowed]);
        c.discard = vec![CardId::Hew, CardId::RaiseShield];
        let events = play(&mut c, 0, None).unwrap();
        assert!(events.contains(&CombatEvent::DeckShuffled));
        assert_eq!(c.hand.len(), 1);
        assert_eq!(c.draw.len(), 1); // 2 reshuffled, 1 drawn
        // Unbowed itself goes to discard AFTER resolving, so it wasn't shuffled in.
        assert_eq!(c.discard, vec![CardId::Unbowed]);
    }

    #[test]
    fn draws_beyond_hand_limit_are_forfeited() {
        // A full hand at draw time is unreachable through Phase 1's card pool
        // (playing always frees a slot first), so this is the one place we
        // unit-test the private helper directly — same file, so tests can.
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)], vec![CardId::Hew; 10]);
        c.draw = vec![CardId::Hew];
        let mut rng = RunRng::new(0);
        let mut evs = Vec::new();
        c.draw_one(&mut rng, &mut evs);
        assert!(evs.is_empty(), "draw at hand limit is forfeited silently");
        assert_eq!(c.hand.len(), 10);
        assert_eq!(c.draw.len(), 1, "the card stays on the draw pile");
    }

    #[test]
    fn whirling_axe_hits_all_living_enemies_only() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 12), enemy(Species::FenRat, 12), enemy(Species::GraveWolf, 40)],
            vec![CardId::WhirlingAxe],
        );
        c.enemies[1].hp = 0;
        let events = play(&mut c, 0, None).unwrap();
        let hits = events.iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 2);
        assert_eq!(c.enemies[0].hp, 4); // the fixture has no Curl Up
        assert_eq!(c.enemies[2].hp, 32);
    }

    #[test]
    fn twin_axes_hits_twice_and_second_hit_fizzles_on_kill() {
        // Two enemies so the first kill doesn't end combat.
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 4), enemy(Species::GraveWolf, 40)],
            vec![CardId::TwinAxes],
        );
        let events = play(&mut c, 0, Some(0)).unwrap();
        let hits: Vec<_> = events.iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .collect();
        assert_eq!(hits.len(), 1, "second hit fizzles on a corpse");
        assert!(events.contains(&CombatEvent::EnemyDied { index: 0 }));
        assert!(c.over.is_none());
    }

    #[test]
    fn twin_axes_double_hits_a_survivor() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)], vec![CardId::TwinAxes]);
        let events = play(&mut c, 0, Some(0)).unwrap();
        let hits = events.iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 2);
        assert_eq!(c.enemies[0].hp, 30);
    }

    #[test]
    fn curl_up_triggers_once_between_hits() {
        let mut c = combat_vs(vec![enemy(Species::BarrowRat, 20)], vec![CardId::TwinAxes]);
        c.enemies[0].statuses.curl_up = Some(4);
        let events = play(&mut c, 0, Some(0)).unwrap();
        // Hit 1: 5 damage to HP (20→15), curl up grants 4 block.
        // Hit 2: 5 damage, 4 blocked, 1 to HP (15→14).
        assert_eq!(c.enemies[0].hp, 14);
        assert_eq!(c.enemies[0].block, 0);
        assert_eq!(c.enemies[0].statuses.curl_up, None);
        assert!(events.contains(&CombatEvent::BlockGained { target: TargetRef::Enemy(0), amount: 4 }));
        assert!(events.contains(&CombatEvent::StatusExpired { target: TargetRef::Enemy(0), status: StatusKind::CurlUp }));
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0), amount: 5, blocked: 4, hp_lost: 1
        }));
    }

    #[test]
    fn skull_splitter_applies_vulnerable_and_amplifies_followups() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)],
                              vec![CardId::SkullSplitter, CardId::Hew]);
        play(&mut c, 0, Some(0)).unwrap(); // 8 damage + Vulnerable 2
        assert_eq!(c.enemies[0].hp, 32);
        assert_eq!(c.enemies[0].statuses.vulnerable, 2);
        play(&mut c, 0, Some(0)).unwrap(); // Hew: floor(6*1.5) = 9
        assert_eq!(c.enemies[0].hp, 23);
    }

    #[test]
    fn thors_wrath_damages_and_debuffs_all() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 20), enemy(Species::FenRat, 20)],
            vec![CardId::ThorsWrath],
        );
        play(&mut c, 0, None).unwrap();
        for e in &c.enemies {
            assert_eq!(e.statuses.vulnerable, 1);
        }
        assert_eq!(c.enemies[0].hp, 16);
        assert_eq!(c.enemies[1].hp, 16);
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core combat`
Expected: the new tests hit the `todo!("Tasks 11–12")` panic (or fail to compile if `draw_one` call syntax is off — fix imports first, then expect panics).

- [ ] **Step 3: Implement the effects** — in `run_effect`, replace the `_ => todo!("Tasks 11–12")` arm with:

```rust
            Effect::DamageAll(base) => {
                let targets: Vec<usize> = self.living().collect();
                for i in targets {
                    if self.over.is_some() {
                        break;
                    }
                    if self.enemies[i].alive() {
                        self.attack_enemy(i, base, events);
                    }
                }
            }
            Effect::ApplyVulnerable(n) => {
                if let Some(t) = target {
                    if self.enemies[t].alive() {
                        self.enemies[t].statuses.vulnerable += n;
                        events.push(CombatEvent::StatusApplied {
                            target: TargetRef::Enemy(t),
                            status: StatusKind::Vulnerable,
                            amount: n as i32,
                        });
                    }
                }
            }
            Effect::ApplyVulnerableAll(n) => {
                let targets: Vec<usize> = self.living().collect();
                for i in targets {
                    self.enemies[i].statuses.vulnerable += n;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Enemy(i),
                        status: StatusKind::Vulnerable,
                        amount: n as i32,
                    });
                }
            }
            Effect::Draw(n) => {
                for _ in 0..n {
                    self.draw_one(rng, events);
                }
            }
            _ => todo!("Task 12"),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core combat`
Expected: all pass (the strength/copy/power tests arrive in Task 12).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): draw, AoE, multi-hit, Curl Up, and Vulnerable effects"
```

### Task 12: Card effects — Strength, Rising Fury, Powers, Enrage, Victory

**Files:**
- Modify: `crates/helheim_core/src/combat.rs`

- [ ] **Step 1: Write the failing tests** — append inside `mod tests`:

```rust
    #[test]
    fn surge_of_rage_gives_temporary_strength() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)],
                              vec![CardId::SurgeOfRage, CardId::Hew]);
        let events = play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.statuses.strength, 2);
        assert_eq!(c.player.statuses.strength_down, 2);
        assert!(events.contains(&CombatEvent::StatusApplied {
            target: TargetRef::Player, status: StatusKind::Strength, amount: 2
        }));
        play(&mut c, 0, Some(0)).unwrap(); // Hew: 6+2 = 8
        assert_eq!(c.enemies[0].hp, 32);
    }

    #[test]
    fn berserkergang_is_consumed_and_strength_persists() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)],
                              vec![CardId::Berserkergang, CardId::Hew]);
        play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.statuses.strength, 2);
        assert_eq!(c.player.statuses.strength_down, 0);
        assert!(c.discard.is_empty(), "powers are consumed, not discarded");
        play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 32);
    }

    #[test]
    fn rising_fury_adds_a_copy_to_discard() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)], vec![CardId::RisingFury]);
        let energy_before = c.player.energy;
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.player.energy, energy_before, "Rising Fury costs 0");
        assert_eq!(c.enemies[0].hp, 34);
        // Copy added during resolution + the played card itself.
        assert_eq!(c.discard, vec![CardId::RisingFury, CardId::RisingFury]);
        assert!(events.contains(&CombatEvent::CardAddedToDiscard { card: CardId::RisingFury }));
    }

    #[test]
    fn enrage_triggers_on_skills_and_updates_intent() {
        let mut c = combat_vs(vec![enemy(Species::ForestTroll, 84)],
                              vec![CardId::RaiseShield, CardId::Hew]);
        c.enemies[0].statuses.enrage = 2;
        c.enemies[0].next_move = EnemyMove::Rush;
        let events = play(&mut c, 0, None).unwrap(); // skill
        assert_eq!(c.enemies[0].statuses.strength, 2);
        assert!(events.contains(&CombatEvent::StatusApplied {
            target: TargetRef::Enemy(0), status: StatusKind::Strength, amount: 2
        }));
        assert!(events.contains(&CombatEvent::IntentSet {
            index: 0,
            intent: IntentKind::Attack { damage: 16, hits: 1 }, // 14 + 2
        }));
        let events = play(&mut c, 0, Some(0)).unwrap(); // attack: no enrage
        assert_eq!(c.enemies[0].statuses.strength, 2);
        assert!(!events.iter().any(|e| matches!(e,
            CombatEvent::StatusApplied { status: StatusKind::Strength, .. })));
    }

    #[test]
    fn killing_the_last_enemy_wins_and_locks_the_combat() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 5)], vec![CardId::Hew, CardId::Hew]);
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.over, Some(Outcome::Victory));
        assert!(events.contains(&CombatEvent::EnemyDied { index: 0 }));
        assert!(events.contains(&CombatEvent::Victory));
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::CombatOver));
        let mut rng = RunRng::new(0);
        assert_eq!(c.apply(&mut rng, Action::EndTurn), Err(IllegalAction::CombatOver));
    }

    #[test]
    fn skull_splitter_kill_skips_the_vulnerable_application() {
        // Victory stops remaining effects: the kill comes first in the list.
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 8)], vec![CardId::SkullSplitter]);
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.over, Some(Outcome::Victory));
        assert!(!events.iter().any(|e| matches!(e,
            CombatEvent::StatusApplied { status: StatusKind::Vulnerable, .. })));
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core combat`
Expected: new tests panic on `todo!("Task 12")`.

- [ ] **Step 3: Implement the remaining effects** — in `run_effect`, replace the `_ => todo!("Task 12")` arm with:

```rust
            Effect::GainStrength(n) => {
                self.player.statuses.strength += n;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Strength,
                    amount: n,
                });
            }
            Effect::GainTempStrength(n) => {
                self.player.statuses.strength += n;
                self.player.statuses.strength_down += n.max(0) as u32;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Strength,
                    amount: n,
                });
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::StrengthDown,
                    amount: n,
                });
            }
            Effect::AddCopyToDiscard => {
                self.discard.push(card);
                events.push(CombatEvent::CardAddedToDiscard { card });
            }
```

(after this, `run_effect`'s match is exhaustive — remove any leftover `_` arm).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core combat`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): strength buffs, card copies, power consumption, enrage"
```

### Task 13: EndTurn — discard, ticks, enemy turns, Ritual, next intents

**Files:**
- Modify: `crates/helheim_core/src/combat.rs`

The locked sequence (see Type Reference; tests below encode it):

1. Hand → discard (`HandDiscarded`).
2. Player end-of-turn: Strength Down fires (lose Strength, status clears).
3. Player duration tick (Vulnerable/Weak −1; `StatusTicked`/`StatusExpired`).
   *Known simplification:* with the round structure player-then-enemies, a
   freshly-applied player Vulnerable amplifies one full enemy round; verify
   against the StS wiki during implementation and, if it disagrees, change
   spec + tests + code in one commit.
4. Each living enemy, in spawn order: its block resets (`BlockReset`), it
   executes its move (`EnemyMoved` + effects), then its end-of-turn: Ritual
   (skipped the turn it was applied via `ritual_fresh`), then its duration
   tick. Move is pushed to its history. If the player dies: `PlayerDied`,
   combat over, **stop immediately** (no further enemies, no intents).
5. All living enemies roll next moves; `IntentSet` per enemy.
6. New player turn: `turn += 1`, player `BlockReset`, energy → 3
   (`EnergySet`), `TurnStarted`, draw 5.

- [ ] **Step 1: Write the failing tests** — append inside `mod tests`:

```rust
    fn end_turn(c: &mut CombatState, seed: u64) -> Vec<CombatEvent> {
        let mut rng = RunRng::new(seed);
        c.apply(&mut rng, Action::EndTurn).unwrap()
    }

    #[test]
    fn end_turn_discards_hand_and_refills_everything() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)],
                              vec![CardId::Hew, CardId::RaiseShield]);
        c.draw = starter_deck();
        c.player.energy = 0;
        c.player.block = 7;
        let events = end_turn(&mut c, 1);
        assert!(events.contains(&CombatEvent::HandDiscarded));
        assert_eq!(c.turn, 2);
        assert_eq!(c.player.energy, 3);
        assert_eq!(c.player.block, 0, "player block expires at turn start");
        assert_eq!(c.hand.len(), 5);
        assert!(c.discard.contains(&CardId::RaiseShield));
        assert!(events.contains(&CombatEvent::TurnStarted { turn: 2 }));
        assert!(events.iter().any(|e| matches!(e, CombatEvent::IntentSet { .. })));
    }

    #[test]
    fn enemy_attacks_through_block() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        c.player.block = 4; // DarkStrike hits 6: 4 blocked, 2 HP lost
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 78);
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Player, amount: 6, blocked: 4, hp_lost: 2
        }));
        assert!(events.contains(&CombatEvent::EnemyMoved {
            index: 0, mv: EnemyMove::DarkStrike
        }));
    }

    #[test]
    fn ritual_skips_its_first_turn_then_scales_6_9_12() {
        // Fresh chanter exactly as a real fight starts.
        let mut c = combat_vs(vec![Enemy {
            history: vec![],
            next_move: EnemyMove::Chant,
            ..enemy(Species::DraugrChanter, 54)
        }], vec![]);
        let hp0 = c.player.hp;
        end_turn(&mut c, 1); // chant; ritual fresh: no strength yet
        assert_eq!(c.player.hp, hp0);
        assert_eq!(c.enemies[0].statuses.ritual, 3);
        assert_eq!(c.enemies[0].statuses.strength, 0);
        end_turn(&mut c, 2); // attacks 6; then end-of-its-turn: +3
        assert_eq!(c.player.hp, hp0 - 6);
        assert_eq!(c.enemies[0].statuses.strength, 3);
        end_turn(&mut c, 3); // attacks 9
        assert_eq!(c.player.hp, hp0 - 15);
        end_turn(&mut c, 4); // attacks 12
        assert_eq!(c.player.hp, hp0 - 27);
    }

    #[test]
    fn fen_rat_spittle_weakens_the_player() {
        let mut c = combat_vs(vec![Enemy {
            next_move: EnemyMove::Spittle,
            ..enemy(Species::FenRat, 14)
        }], vec![CardId::Hew]);
        end_turn(&mut c, 1);
        assert_eq!(c.player.statuses.weak, 2);
        // Weak player: Hew deals floor(6*0.75) = 4.
        play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 10);
    }

    #[test]
    fn player_durations_tick_at_end_of_player_turn() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        c.player.statuses.vulnerable = 2;
        // End turn: tick 2→1 (before the enemy acts), enemy hits 6*1.5 = 9.
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 71);
        assert!(events.contains(&CombatEvent::StatusTicked {
            target: TargetRef::Player, status: StatusKind::Vulnerable, remaining: 1
        }));
        // Next end turn: tick 1→0, enemy hits plain 6.
        let events = end_turn(&mut c, 2);
        assert_eq!(c.player.hp, 65);
        assert!(events.contains(&CombatEvent::StatusExpired {
            target: TargetRef::Player, status: StatusKind::Vulnerable
        }));
    }

    #[test]
    fn enemy_durations_tick_at_end_of_its_own_turn() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)], vec![]);
        c.enemies[0].statuses.vulnerable = 2;
        c.enemies[0].next_move = EnemyMove::Bellow;
        let events = end_turn(&mut c, 1);
        assert_eq!(c.enemies[0].statuses.vulnerable, 1);
        assert!(events.contains(&CombatEvent::StatusTicked {
            target: TargetRef::Enemy(0), status: StatusKind::Vulnerable, remaining: 1
        }));
    }

    #[test]
    fn strength_down_fires_at_end_of_player_turn() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)],
                              vec![CardId::SurgeOfRage]);
        play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.statuses.strength, 2);
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.statuses.strength, 0);
        assert_eq!(c.player.statuses.strength_down, 0);
        assert!(events.contains(&CombatEvent::StatusApplied {
            target: TargetRef::Player, status: StatusKind::Strength, amount: -2
        }));
        assert!(events.contains(&CombatEvent::StatusExpired {
            target: TargetRef::Player, status: StatusKind::StrengthDown
        }));
    }

    #[test]
    fn thrash_attacks_and_blocks_and_bellow_buffs() {
        let mut c = combat_vs(vec![Enemy {
            next_move: EnemyMove::Thrash,
            ..enemy(Species::GraveWolf, 40)
        }], vec![]);
        end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 73); // 7 damage
        // Wolf's block was gained AFTER its block reset, so it shows 5 now.
        assert_eq!(c.enemies[0].block, 5);
        // Next turn the wolf's own block resets first.
        c.enemies[0].next_move = EnemyMove::Bellow;
        end_turn(&mut c, 2);
        assert_eq!(c.enemies[0].block, 6, "old 5 reset, Bellow grants 6");
        assert_eq!(c.enemies[0].statuses.strength, 3);
    }

    #[test]
    fn troll_bellow_applies_enrage() {
        let mut c = combat_vs(vec![Enemy {
            history: vec![],
            next_move: EnemyMove::TrollBellow,
            ..enemy(Species::ForestTroll, 84)
        }], vec![]);
        end_turn(&mut c, 1);
        assert_eq!(c.enemies[0].statuses.enrage, 2);
    }

    #[test]
    fn player_death_stops_the_round_immediately() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50), enemy(Species::DraugrChanter, 50)],
            vec![],
        );
        c.player.hp = 3; // first DarkStrike (6) kills
        let events = end_turn(&mut c, 1);
        assert_eq!(c.over, Some(Outcome::Defeat));
        assert!(events.contains(&CombatEvent::PlayerDied));
        let hits = events.iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 1, "second enemy never acts");
        assert!(!events.iter().any(|e| matches!(e, CombatEvent::TurnStarted { turn: 2 })));
    }

    #[test]
    fn dead_enemies_are_skipped() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50), enemy(Species::DraugrChanter, 50)],
            vec![],
        );
        c.enemies[0].hp = 0;
        let events = end_turn(&mut c, 1);
        let hits = events.iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 1);
        assert!(!events.iter().any(|e| matches!(e,
            CombatEvent::IntentSet { index: 0, .. })));
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core combat`
Expected: compile error (`end_turn` not defined on `CombatState`).

- [ ] **Step 3: Implement `end_turn`, `enemy_move`, `enemy_attack`** — add inside `impl CombatState`:

```rust
    fn end_turn(&mut self, rng: &mut RunRng) -> Vec<CombatEvent> {
        let mut events = Vec::new();

        // 1. Discard the hand.
        self.discard.append(&mut self.hand);
        events.push(CombatEvent::HandDiscarded);

        // 2. Player end-of-turn statuses.
        let sd = self.player.statuses.strength_down;
        if sd > 0 {
            self.player.statuses.strength -= sd as i32;
            self.player.statuses.strength_down = 0;
            events.push(CombatEvent::StatusApplied {
                target: TargetRef::Player,
                status: StatusKind::Strength,
                amount: -(sd as i32),
            });
            events.push(CombatEvent::StatusExpired {
                target: TargetRef::Player,
                status: StatusKind::StrengthDown,
            });
        }

        // 3. Player duration tick.
        Self::push_tick_events(TargetRef::Player, self.player.statuses.tick_durations(), &mut events);

        // 4. Enemy turns, in spawn order.
        for i in 0..self.enemies.len() {
            if !self.enemies[i].alive() {
                continue;
            }
            self.enemies[i].block = 0;
            events.push(CombatEvent::BlockReset { target: TargetRef::Enemy(i) });

            let mv = self.enemies[i].next_move;
            events.push(CombatEvent::EnemyMoved { index: i, mv });
            self.enemy_move(i, mv, &mut events);
            if self.over.is_some() {
                return events; // player died mid-round
            }

            let e = &mut self.enemies[i];
            if e.statuses.ritual > 0 {
                if e.statuses.ritual_fresh {
                    e.statuses.ritual_fresh = false;
                } else {
                    let gain = e.statuses.ritual as i32;
                    e.statuses.strength += gain;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Enemy(i),
                        status: StatusKind::Strength,
                        amount: gain,
                    });
                }
            }
            let ticks = e.statuses.tick_durations();
            Self::push_tick_events(TargetRef::Enemy(i), ticks, &mut events);
            self.enemies[i].history.push(mv);
        }

        // 5. Roll next moves and show intents.
        for i in 0..self.enemies.len() {
            if !self.enemies[i].alive() {
                continue;
            }
            self.enemies[i].next_move =
                roll_move(self.enemies[i].species, &self.enemies[i].history, rng);
            events.push(CombatEvent::IntentSet { index: i, intent: self.intent_of(i) });
        }

        // 6. New player turn.
        self.turn += 1;
        self.player.block = 0;
        events.push(CombatEvent::BlockReset { target: TargetRef::Player });
        self.player.energy = ENERGY_PER_TURN;
        events.push(CombatEvent::EnergySet { energy: ENERGY_PER_TURN });
        events.push(CombatEvent::TurnStarted { turn: self.turn });
        for _ in 0..DRAW_PER_TURN {
            self.draw_one(rng, &mut events);
        }
        events
    }

    fn push_tick_events(
        target: TargetRef,
        ticks: Vec<(StatusKind, u32)>,
        events: &mut Vec<CombatEvent>,
    ) {
        for (status, remaining) in ticks {
            events.push(if remaining == 0 {
                CombatEvent::StatusExpired { target, status }
            } else {
                CombatEvent::StatusTicked { target, status, remaining }
            });
        }
    }

    fn enemy_move(&mut self, i: usize, mv: EnemyMove, events: &mut Vec<CombatEvent>) {
        match mv {
            EnemyMove::Chant => {
                let e = &mut self.enemies[i];
                e.statuses.ritual += 3;
                e.statuses.ritual_fresh = true;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i), status: StatusKind::Ritual, amount: 3,
                });
            }
            EnemyMove::DarkStrike => self.enemy_attack(i, 6, events),
            EnemyMove::Chomp => self.enemy_attack(i, 11, events),
            EnemyMove::Thrash => {
                self.enemy_attack(i, 7, events);
                if self.over.is_none() {
                    self.enemies[i].block += 5;
                    events.push(CombatEvent::BlockGained {
                        target: TargetRef::Enemy(i), amount: 5,
                    });
                }
            }
            EnemyMove::Bellow => {
                let e = &mut self.enemies[i];
                e.statuses.strength += 3;
                e.block += 6;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i), status: StatusKind::Strength, amount: 3,
                });
                events.push(CombatEvent::BlockGained { target: TargetRef::Enemy(i), amount: 6 });
            }
            EnemyMove::Bite => {
                let base = self.enemies[i].bite_damage;
                self.enemy_attack(i, base, events);
            }
            EnemyMove::Grow => {
                self.enemies[i].statuses.strength += 3;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i), status: StatusKind::Strength, amount: 3,
                });
            }
            EnemyMove::Spittle => {
                self.player.statuses.weak += 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player, status: StatusKind::Weak, amount: 2,
                });
            }
            EnemyMove::TrollBellow => {
                self.enemies[i].statuses.enrage += 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i), status: StatusKind::Enrage, amount: 2,
                });
            }
            EnemyMove::Rush => self.enemy_attack(i, 14, events),
            EnemyMove::SkullBash => {
                self.enemy_attack(i, 6, events);
                if self.over.is_none() {
                    self.player.statuses.vulnerable += 2;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Player, status: StatusKind::Vulnerable, amount: 2,
                    });
                }
            }
        }
    }

    fn enemy_attack(&mut self, i: usize, base: u32, events: &mut Vec<CombatEvent>) {
        let e = &self.enemies[i];
        let dmg = attack_damage(
            base,
            e.statuses.strength,
            e.statuses.weak > 0,
            self.player.statuses.vulnerable > 0,
        );
        let out = soak(&mut self.player.block, &mut self.player.hp, dmg);
        events.push(CombatEvent::DamageDealt {
            target: TargetRef::Player,
            amount: dmg,
            blocked: out.blocked,
            hp_lost: out.hp_lost,
        });
        if self.player.hp == 0 {
            self.over = Some(Outcome::Defeat);
            events.push(CombatEvent::PlayerDied);
        }
    }
```

- [ ] **Step 4: Run the full module and fix anything that drifted**

Run: `cargo test -p helheim_core`
Expected: all tests pass (rng, cards, statuses, enemies, combat).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): full turn cycle with enemy moves, Ritual, ticks, intents"
```
### Task 14: `run.rs` — gauntlet stages, rewards, stats, HP carry

**Files:**
- Create: `crates/helheim_core/src/run.rs`
- Modify: `crates/helheim_core/src/lib.rs` (add `pub mod run;`)

- [ ] **Step 1: Write the failing tests** — create `run.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::REWARD_POOL;
    use crate::combat::Action;
    use crate::enemies::Species;

    /// Reach into the public combat state and rig every enemy to 1 HP, then
    /// bot through: play the first affordable attack at the first living
    /// enemy, else end turn. Wins fast and exercises the real API.
    fn win_current_fight(run: &mut RunState) {
        for e in &mut run.combat.as_mut().unwrap().enemies {
            e.hp = 1;
        }
        for _ in 0..200 {
            if run.combat.is_none() {
                return; // stage advanced
            }
            let action = {
                let c = run.combat.as_ref().unwrap();
                let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
                c.hand
                    .iter()
                    .enumerate()
                    .find_map(|(i, card)| {
                        let spec = card.spec();
                        if spec.cost > c.player.energy {
                            return None;
                        }
                        match spec.targeting {
                            crate::cards::Targeting::SingleEnemy => target.map(|t| {
                                Action::PlayCard { hand_index: i, target: Some(t) }
                            }),
                            _ => Some(Action::PlayCard { hand_index: i, target: None }),
                        }
                    })
                    .unwrap_or(Action::EndTurn)
            };
            run.apply(action).unwrap();
        }
        panic!("rigged fight did not end in 200 actions");
    }

    #[test]
    fn new_run_starts_at_fight_1_with_starter_deck() {
        let run = RunState::new(7);
        assert_eq!(run.stage, Stage::Fight(1));
        assert_eq!(run.master_deck.len(), 10);
        assert_eq!(run.hp, 80);
        assert!(run.combat.is_none());
    }

    #[test]
    fn fight_1_is_chanter_or_wolf_fight_2_rats_fight_3_troll() {
        let mut run = RunState::new(7);
        run.begin_fight().unwrap();
        {
            let c = run.combat.as_ref().unwrap();
            assert_eq!(c.enemies.len(), 1);
            assert!(matches!(c.enemies[0].species,
                Species::DraugrChanter | Species::GraveWolf));
        }
        win_current_fight(&mut run);
        run.choose_reward(None).unwrap();
        run.begin_fight().unwrap();
        {
            let c = run.combat.as_ref().unwrap();
            let species: Vec<Species> = c.enemies.iter().map(|e| e.species).collect();
            assert_eq!(species, vec![Species::BarrowRat, Species::FenRat]);
        }
        win_current_fight(&mut run);
        run.choose_reward(None).unwrap();
        run.begin_fight().unwrap();
        assert_eq!(run.combat.as_ref().unwrap().enemies[0].species, Species::ForestTroll);
    }

    #[test]
    fn winning_a_fight_offers_3_distinct_pool_cards() {
        let mut run = RunState::new(11);
        run.begin_fight().unwrap();
        win_current_fight(&mut run);
        let Stage::Reward { after_fight, offer } = run.stage else {
            panic!("expected reward stage, got {:?}", run.stage)
        };
        assert_eq!(after_fight, 1);
        assert!(offer.iter().all(|c| REWARD_POOL.contains(c)));
        assert_ne!(offer[0], offer[1]);
        assert_ne!(offer[1], offer[2]);
        assert_ne!(offer[0], offer[2]);
    }

    #[test]
    fn choosing_a_reward_grows_the_master_deck_skipping_does_not() {
        let mut run = RunState::new(11);
        run.begin_fight().unwrap();
        win_current_fight(&mut run);
        let Stage::Reward { offer, .. } = run.stage else { panic!() };
        run.choose_reward(Some(1)).unwrap();
        assert_eq!(run.master_deck.len(), 11);
        assert_eq!(*run.master_deck.last().unwrap(), offer[1]);
        assert_eq!(run.stage, Stage::Fight(2));

        let mut run2 = RunState::new(11);
        run2.begin_fight().unwrap();
        win_current_fight(&mut run2);
        run2.choose_reward(None).unwrap();
        assert_eq!(run2.master_deck.len(), 10);
        assert_eq!(run2.stage, Stage::Fight(2));
    }

    #[test]
    fn reward_errors_in_wrong_stage_or_bad_index() {
        let mut run = RunState::new(3);
        assert_eq!(run.choose_reward(Some(0)), Err(RunError::NotInReward));
        run.begin_fight().unwrap();
        win_current_fight(&mut run);
        assert_eq!(run.choose_reward(Some(9)), Err(RunError::BadIndex));
    }

    #[test]
    fn hp_carries_between_fights() {
        let mut run = RunState::new(5);
        run.begin_fight().unwrap();
        run.combat.as_mut().unwrap().player.hp = 55; // pretend we took 25
        win_current_fight(&mut run);
        assert_eq!(run.hp, 55);
        run.choose_reward(None).unwrap();
        run.begin_fight().unwrap();
        assert_eq!(run.combat.as_ref().unwrap().player.hp, 55);
    }

    #[test]
    fn beating_fight_3_wins_the_run() {
        let mut run = RunState::new(13);
        for fight in 1..=3u8 {
            assert_eq!(run.stage, Stage::Fight(fight));
            run.begin_fight().unwrap();
            win_current_fight(&mut run);
            if fight < 3 {
                run.choose_reward(Some(0)).unwrap();
            }
        }
        assert_eq!(run.stage, Stage::Victory);
        assert_eq!(run.master_deck.len(), 12);
        assert!(run.stats.damage_dealt > 0);
        assert!(run.stats.turns > 0);
    }

    #[test]
    fn dying_sets_defeat_and_zero_hp() {
        let mut run = RunState::new(17);
        run.begin_fight().unwrap();
        run.combat.as_mut().unwrap().player.hp = 1;
        for _ in 0..20 {
            if run.combat.is_none() {
                break;
            }
            run.apply(Action::EndTurn).unwrap();
        }
        assert_eq!(run.stage, Stage::Defeat);
        assert_eq!(run.hp, 0);
    }

    #[test]
    fn begin_fight_guards_stage_and_double_start() {
        let mut run = RunState::new(1);
        run.begin_fight().unwrap();
        assert_eq!(run.begin_fight(), Err(RunError::NotInFight));
    }
}
```

(Why 20 end-turns in the death test: if fight 1 rolled a Draugr Chanter it
chants on turn 1 and only attacks from turn 2 — 1 HP falls on the first
actual attack.)

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim_core run`
Expected: compile error (`RunState` undefined).

- [ ] **Step 3: Implement `RunState`** — prepend to `run.rs`:

```rust
use crate::cards::{starter_deck, CardId, REWARD_POOL};
use crate::combat::{Action, CombatEvent, CombatState, IllegalAction, Outcome, TargetRef};
use crate::enemies::Species;
use crate::rng::RunRng;

pub const STARTING_HP: u32 = 80;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Fight(u8),
    Reward { after_fight: u8, offer: [CardId; 3] },
    Victory,
    Defeat,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct RunStats {
    pub turns: u32,
    pub damage_dealt: u64,
    pub damage_taken: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunError { NotInFight, NotInReward, BadIndex }

/// One whole gauntlet run: master deck, carried HP, stage progression, and
/// the single RNG stream that makes the run reproducible from its seed.
pub struct RunState {
    pub seed: u64,
    rng: RunRng,
    pub master_deck: Vec<CardId>,
    pub hp: u32,
    pub max_hp: u32,
    pub stage: Stage,
    pub combat: Option<CombatState>,
    pub stats: RunStats,
}

impl RunState {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rng: RunRng::new(seed),
            master_deck: starter_deck(),
            hp: STARTING_HP,
            max_hp: STARTING_HP,
            stage: Stage::Fight(1),
            combat: None,
            stats: RunStats::default(),
        }
    }

    fn encounter(&mut self, fight: u8) -> Vec<Species> {
        match fight {
            1 => vec![self.rng.pick(&[Species::DraugrChanter, Species::GraveWolf])],
            2 => vec![Species::BarrowRat, Species::FenRat],
            _ => vec![Species::ForestTroll],
        }
    }

    pub fn begin_fight(&mut self) -> Result<Vec<CombatEvent>, RunError> {
        let Stage::Fight(n) = self.stage else {
            return Err(RunError::NotInFight);
        };
        if self.combat.is_some() {
            return Err(RunError::NotInFight);
        }
        let species = self.encounter(n);
        let (combat, events) =
            CombatState::new(&mut self.rng, &self.master_deck, self.hp, self.max_hp, &species);
        self.combat = Some(combat);
        self.track(&events);
        Ok(events)
    }

    pub fn apply(&mut self, action: Action) -> Result<Vec<CombatEvent>, IllegalAction> {
        let combat = self.combat.as_mut().ok_or(IllegalAction::CombatOver)?;
        let events = combat.apply(&mut self.rng, action)?;
        self.track(&events);

        match self.combat.as_ref().and_then(|c| c.over) {
            Some(Outcome::Victory) => {
                let combat = self.combat.take().expect("combat exists");
                self.hp = combat.player.hp;
                let Stage::Fight(n) = self.stage else { unreachable!("victory outside a fight") };
                if n >= 3 {
                    self.stage = Stage::Victory;
                } else {
                    let offer = self.roll_offer();
                    self.stage = Stage::Reward { after_fight: n, offer };
                }
            }
            Some(Outcome::Defeat) => {
                self.combat = None;
                self.hp = 0;
                self.stage = Stage::Defeat;
            }
            None => {}
        }
        Ok(events)
    }

    pub fn choose_reward(&mut self, pick: Option<usize>) -> Result<(), RunError> {
        let Stage::Reward { after_fight, offer } = self.stage else {
            return Err(RunError::NotInReward);
        };
        if let Some(i) = pick {
            let card = *offer.get(i).ok_or(RunError::BadIndex)?;
            self.master_deck.push(card);
        }
        self.stage = Stage::Fight(after_fight + 1);
        Ok(())
    }

    fn roll_offer(&mut self) -> [CardId; 3] {
        let mut pool = REWARD_POOL.to_vec();
        self.rng.shuffle(&mut pool);
        [pool[0], pool[1], pool[2]]
    }

    fn track(&mut self, events: &[CombatEvent]) {
        for ev in events {
            match ev {
                CombatEvent::TurnStarted { .. } => self.stats.turns += 1,
                CombatEvent::DamageDealt { target, amount, .. } => match target {
                    TargetRef::Player => self.stats.damage_taken += u64::from(*amount),
                    TargetRef::Enemy(_) => self.stats.damage_dealt += u64::from(*amount),
                },
                _ => {}
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim_core`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): run state with gauntlet stages, rewards, stats, HP carry"
```

### Task 15: Integration — policy bot, full gauntlet, determinism

**Files:**
- Create: `crates/helheim_core/tests/gauntlet.rs`

This is an *integration* test (separate `tests/` dir): it may only use the
public API, which proves the API is sufficient for a real driver (the Bevy
shell uses exactly the same surface).

- [ ] **Step 1: Write the tests**

```rust
use helheim_core::cards::Targeting;
use helheim_core::combat::{Action, CombatState};
use helheim_core::run::{RunState, Stage};

/// Dumb-but-legal policy: play the first affordable card (targeting the
/// first living enemy when needed), else end the turn.
fn choose_action(c: &CombatState) -> Action {
    let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
    for (i, card) in c.hand.iter().enumerate() {
        let spec = card.spec();
        if spec.cost > c.player.energy {
            continue;
        }
        return match spec.targeting {
            Targeting::SingleEnemy => match target {
                Some(t) => Action::PlayCard { hand_index: i, target: Some(t) },
                None => continue,
            },
            _ => Action::PlayCard { hand_index: i, target: None },
        };
    }
    Action::EndTurn
}

/// Drive a whole run with the policy bot. Returns the finished run and the
/// debug-formatted event log (the determinism fingerprint).
fn run_bot(seed: u64) -> (RunState, Vec<String>) {
    let mut run = RunState::new(seed);
    let mut log = Vec::new();
    for _ in 0..10_000 {
        match run.stage {
            Stage::Fight(_) => {
                if run.combat.is_none() {
                    for e in run.begin_fight().unwrap() {
                        log.push(format!("{e:?}"));
                    }
                } else {
                    let action = choose_action(run.combat.as_ref().unwrap());
                    for e in run.apply(action).unwrap() {
                        log.push(format!("{e:?}"));
                    }
                }
            }
            Stage::Reward { .. } => run.choose_reward(Some(0)).unwrap(),
            Stage::Victory | Stage::Defeat => return (run, log),
        }
    }
    panic!("bot did not finish a run in 10k steps (seed {seed})");
}

#[test]
fn every_seed_reaches_a_terminal_stage_with_consistent_state() {
    for seed in 0..25u64 {
        let (run, log) = run_bot(seed);
        assert!(!log.is_empty());
        match run.stage {
            Stage::Victory => {
                assert!(run.hp > 0, "seed {seed}: won with 0 hp");
                assert_eq!(run.master_deck.len(), 12, "two rewards were picked");
                assert!(run.stats.damage_dealt > 0);
            }
            Stage::Defeat => assert_eq!(run.hp, 0, "seed {seed}: lost with hp left"),
            other => panic!("seed {seed}: non-terminal stage {other:?}"),
        }
        assert!(run.stats.turns > 0);
        assert!(run.combat.is_none());
    }
}

#[test]
fn same_seed_same_run_byte_for_byte() {
    let (run_a, log_a) = run_bot(424242);
    let (run_b, log_b) = run_bot(424242);
    assert_eq!(log_a, log_b);
    assert_eq!(run_a.stage, run_b.stage);
    assert_eq!(run_a.hp, run_b.hp);
    assert_eq!(run_a.master_deck, run_b.master_deck);
    assert_eq!(run_a.stats, run_b.stats);
}

#[test]
fn different_seeds_diverge() {
    let (_, log_a) = run_bot(1);
    let (_, log_b) = run_bot(2);
    assert_ne!(log_a, log_b);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p helheim_core --test gauntlet`
Expected: 3 passed. These should pass immediately if Tasks 9–14 are correct
— any failure here is a real engine bug (most likely: a `continue` inside the
`match` in `choose_action` needs the loop labeled — if the compiler rejects
`continue` inside the match arm, restructure with a labeled loop
`'hand: for (i, card) ...` and `continue 'hand`).

- [ ] **Step 3: Verify the whole core suite + lints are clean**

```bash
cargo test -p helheim_core
cargo clippy -p helheim_core --all-targets -- -D warnings
cargo fmt --all
git diff --stat  # fmt may have reflowed files; review briefly
```

Expected: tests green, clippy clean (fix any warnings it raises — typical:
`needless_range_loop`, `len_zero`; keep fixes minimal and re-run tests).

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "test(core): policy-bot gauntlet integration and determinism tests"
```
### Task 16: Shell scaffold — bin crate, window, theme, menu

**Files:**
- Modify: `Cargo.toml` (workspace members)
- Create: `crates/helheim/Cargo.toml`
- Create: `crates/helheim/src/main.rs`
- Create: `crates/helheim/src/theme.rs`
- Create: `crates/helheim/src/screens/mod.rs`
- Create: `crates/helheim/src/screens/menu.rs`
- Create: `crates/helheim/src/anim.rs` (stub — Task 17 replaces it)
- Create: `crates/helheim/src/screens/combat.rs` (stub — Task 18 replaces it)
- Create: `crates/helheim/src/screens/reward.rs` (stub — Task 19 replaces it)
- Create: `crates/helheim/src/screens/end.rs` (stub — Task 19 replaces it)
- Create: `crates/helheim/assets/fonts/FiraSans-Regular.ttf` (downloaded)

There is no TDD loop for window code; the smoke test covers state wiring and
the rest is verified by running the app.

- [ ] **Step 1: Add the crate to the workspace** — in root `Cargo.toml`:

```toml
members = ["crates/helheim_core", "crates/helheim"]
```

- [ ] **Step 2: Write `crates/helheim/Cargo.toml`**

```toml
[package]
name = "helheim"
version = "0.1.0"
edition = "2021"

[dependencies]
helheim_core = { path = "../helheim_core" }
bevy = "0.18"
rand = "0.9"

[features]
# cargo run -p helheim --features dev  → much faster rebuilds
dev = ["bevy/dynamic_linking"]
```

- [ ] **Step 3: Download the font (SIL OFL licensed)**

```bash
mkdir -p crates/helheim/assets/fonts
curl -fL -o crates/helheim/assets/fonts/FiraSans-Regular.ttf \
  https://github.com/mozilla/Fira/raw/master/ttf/FiraSans-Regular.ttf \
|| curl -fL -o crates/helheim/assets/fonts/FiraSans-Regular.ttf \
  https://github.com/google/fonts/raw/main/ofl/firasans/FiraSans-Regular.ttf
file crates/helheim/assets/fonts/FiraSans-Regular.ttf  # expect: TrueType font
```

- [ ] **Step 4: Write `crates/helheim/src/main.rs`**

```rust
use bevy::prelude::*;
use helheim_core::run::RunState;

mod anim;
mod screens;
mod theme;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    Combat,
    Reward,
    Victory,
    GameOver,
}

/// Seed from `--seed <n>`, if given. "Again" reuses it (reproducible runs);
/// otherwise every run gets a fresh random seed.
#[derive(Resource)]
pub struct CliSeed(pub Option<u64>);

impl CliSeed {
    pub fn next_seed(&self) -> u64 {
        self.0.unwrap_or_else(rand::random::<u64>)
    }
}

#[derive(Resource)]
pub struct Session {
    pub run: RunState,
}

fn parse_seed() -> Option<u64> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == "--seed")
        .and_then(|w| w[1].parse().ok())
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Helheim".into(),
                resolution: (1280., 720.).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(CliSeed(parse_seed()))
        .init_state::<AppState>()
        .add_plugins((
            theme::ThemePlugin,
            anim::AnimPlugin,
            screens::menu::MenuPlugin,
            screens::combat::CombatScreenPlugin,
            screens::reward::RewardPlugin,
            screens::end::EndScreensPlugin,
        ))
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    /// Headless smoke test: the state machine wires up and transitions.
    #[test]
    fn app_state_machine_transitions() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<AppState>();
        app.update();
        assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::Menu);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Combat);
        app.update();
        assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::Combat);
    }
}
```

(Until Tasks 17–19 exist, stub the missing modules so this compiles — write
`screens/combat.rs`, `screens/reward.rs`, `screens/end.rs` each as an empty
plugin now, replaced by their real tasks:)

```rust
// screens/combat.rs (STUB — replaced in Task 18)
use bevy::prelude::*;
pub struct CombatScreenPlugin;
impl Plugin for CombatScreenPlugin {
    fn build(&self, _app: &mut App) {}
}
```

```rust
// screens/reward.rs (STUB — replaced in Task 19)
use bevy::prelude::*;
pub struct RewardPlugin;
impl Plugin for RewardPlugin {
    fn build(&self, _app: &mut App) {}
}
```

```rust
// screens/end.rs (STUB — replaced in Task 19)
use bevy::prelude::*;
pub struct EndScreensPlugin;
impl Plugin for EndScreensPlugin {
    fn build(&self, _app: &mut App) {}
}
```

```rust
// anim.rs (STUB — replaced in Task 17)
use bevy::prelude::*;
pub struct AnimPlugin;
impl Plugin for AnimPlugin {
    fn build(&self, _app: &mut App) {}
}
```

- [ ] **Step 5: Write `crates/helheim/src/theme.rs`**

```rust
use bevy::prelude::*;

pub const BG: Color = Color::srgb(0.07, 0.07, 0.10);
pub const PANEL: Color = Color::srgb(0.14, 0.14, 0.19);
pub const PANEL_DIM: Color = Color::srgb(0.10, 0.10, 0.13);
pub const PANEL_HOVER: Color = Color::srgb(0.22, 0.22, 0.30);
pub const ACCENT: Color = Color::srgb(0.78, 0.22, 0.20);
pub const TEXT: Color = Color::srgb(0.92, 0.90, 0.85);
pub const TEXT_DIM: Color = Color::srgb(0.58, 0.58, 0.58);
pub const HP_COLOR: Color = Color::srgb(0.85, 0.30, 0.28);
pub const BLOCK_COLOR: Color = Color::srgb(0.38, 0.58, 0.92);
pub const ENERGY_COLOR: Color = Color::srgb(0.95, 0.76, 0.22);

#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(BG))
            .add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.insert_resource(UiFont(assets.load("fonts/FiraSans-Regular.ttf")));
}

/// Text bundle helper: every label in the game goes through this.
pub fn text(
    font: &UiFont,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> (Text, TextFont, TextColor) {
    (
        Text::new(s),
        TextFont {
            font: font.0.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

/// A standard button: panel background, padding, child label.
pub fn button(
    commands: &mut ChildSpawnerCommands,
    font: &UiFont,
    marker: impl Bundle,
    label: &str,
) {
    commands
        .spawn((
            marker,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(24.), Val::Px(10.)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|b| {
            b.spawn(text(font, label, 24., TEXT));
        });
}
```

(API drift candidates: `ChildSpawnerCommands` was `ChildBuilder` before Bevy
0.16 — if the compiler rejects it, use the type the `with_children` closure
provides, per the migration guide.)

- [ ] **Step 6: Write `crates/helheim/src/screens/mod.rs` and `menu.rs`**

```rust
// screens/mod.rs
pub mod combat;
pub mod end;
pub mod menu;
pub mod reward;
```

```rust
// screens/menu.rs
use bevy::prelude::*;
use helheim_core::run::RunState;

use crate::theme::{self, UiFont};
use crate::{AppState, CliSeed, Session};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), spawn_menu)
            .add_systems(OnExit(AppState::Menu), despawn_menu)
            .add_systems(Update, begin_button.run_if(in_state(AppState::Menu)));
    }
}

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct BeginButton;

fn spawn_menu(mut commands: Commands, font: Res<UiFont>) {
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(28.),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn(theme::text(&font, "HELHEIM", 80., theme::ACCENT));
            p.spawn(theme::text(&font, "the barrow road", 26., theme::TEXT_DIM));
            theme::button(p, &font, BeginButton, "Begin the Descent");
        });
}

fn despawn_menu(mut commands: Commands, q: Query<Entity, With<MenuRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn begin_button(
    mut commands: Commands,
    cli: Res<CliSeed>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<BeginButton>),
    >,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, mut bg) in &mut q {
        match interaction {
            Interaction::Pressed => {
                commands.insert_resource(Session {
                    run: RunState::new(cli.next_seed()),
                });
                next.set(AppState::Combat);
            }
            Interaction::Hovered => *bg = BackgroundColor(theme::PANEL_HOVER),
            Interaction::None => *bg = BackgroundColor(theme::PANEL),
        }
    }
}
```

- [ ] **Step 7: Build, run the smoke test, then run the app**

```bash
cargo test -p helheim          # first Bevy build: several minutes
cargo run -p helheim --features dev
```

Expected: smoke test passes; a 1280×720 "Helheim" window opens with the title,
subtitle, and a hoverable "Begin the Descent" button. Pressing it transitions
to a blank screen (Combat is a stub) — that's correct for now. Close the
window. **If the build fails on Bevy API names, consult the 0.17/0.18
migration guides and adapt — keep the structure.**

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(shell): Bevy app scaffold with menu, theme, states, smoke test"
```

### Task 17: `anim.rs` — DisplayState, event replay, beats, floaters

**Files:**
- Modify: `crates/helheim/src/anim.rs` (replace the stub entirely)

`DisplayState` is the UI's source of truth. It is mutated ONLY by replaying
core events through the queue, one "beat" at a time, so the player watches
causality in order while the core has long since finished resolving.
`apply_event` is pure data-mapping — that's why it gets real unit tests.

- [ ] **Step 1: Write the failing tests** — at the bottom of the new `anim.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use helheim_core::cards::CardId;
    use helheim_core::combat::{CombatEvent, IntentKind, Outcome, TargetRef};
    use helheim_core::statuses::StatusKind;

    fn fixture() -> DisplayState {
        DisplayState {
            player_hp: 80,
            player_max_hp: 80,
            player_block: 0,
            energy: 3,
            statuses: Default::default(),
            hand: vec![CardId::Hew, CardId::RaiseShield],
            draw_count: 5,
            discard_count: 3,
            enemies: vec![EnemyView {
                name: "Grave Wolf",
                hp: 40,
                max_hp: 44,
                block: 0,
                statuses: Default::default(),
                intent: None,
                alive: true,
            }],
            outcome: None,
            turn: 1,
        }
    }

    #[test]
    fn damage_event_updates_hp_and_block() {
        let mut ds = fixture();
        ds.enemies[0].block = 3;
        apply_event(&mut ds, &CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0), amount: 8, blocked: 3, hp_lost: 5,
        });
        assert_eq!(ds.enemies[0].hp, 35);
        assert_eq!(ds.enemies[0].block, 0);

        apply_event(&mut ds, &CombatEvent::DamageDealt {
            target: TargetRef::Player, amount: 6, blocked: 0, hp_lost: 6,
        });
        assert_eq!(ds.player_hp, 74);
    }

    #[test]
    fn card_flow_events_track_zone_counts() {
        let mut ds = fixture();
        apply_event(&mut ds, &CombatEvent::CardPlayed { card: CardId::Hew, hand_index: 0 });
        assert_eq!(ds.hand, vec![CardId::RaiseShield]);
        assert_eq!(ds.discard_count, 4);

        apply_event(&mut ds, &CombatEvent::CardDrawn { card: CardId::TwinAxes });
        assert_eq!(ds.hand, vec![CardId::RaiseShield, CardId::TwinAxes]);
        assert_eq!(ds.draw_count, 4);

        apply_event(&mut ds, &CombatEvent::HandDiscarded);
        assert!(ds.hand.is_empty());
        assert_eq!(ds.discard_count, 6);

        apply_event(&mut ds, &CombatEvent::DeckShuffled);
        assert_eq!(ds.draw_count, 10);
        assert_eq!(ds.discard_count, 0);
    }

    #[test]
    fn powers_do_not_join_the_discard_count() {
        let mut ds = fixture();
        ds.hand = vec![CardId::Berserkergang];
        apply_event(&mut ds, &CombatEvent::CardPlayed { card: CardId::Berserkergang, hand_index: 0 });
        assert!(ds.hand.is_empty());
        assert_eq!(ds.discard_count, 3, "powers are consumed");
    }

    #[test]
    fn status_events_mutate_the_right_creature() {
        let mut ds = fixture();
        apply_event(&mut ds, &CombatEvent::StatusApplied {
            target: TargetRef::Enemy(0), status: StatusKind::Vulnerable, amount: 2,
        });
        assert_eq!(ds.enemies[0].statuses.vulnerable, 2);
        apply_event(&mut ds, &CombatEvent::StatusTicked {
            target: TargetRef::Enemy(0), status: StatusKind::Vulnerable, remaining: 1,
        });
        assert_eq!(ds.enemies[0].statuses.vulnerable, 1);
        apply_event(&mut ds, &CombatEvent::StatusExpired {
            target: TargetRef::Enemy(0), status: StatusKind::Vulnerable,
        });
        assert_eq!(ds.enemies[0].statuses.vulnerable, 0);

        apply_event(&mut ds, &CombatEvent::StatusApplied {
            target: TargetRef::Player, status: StatusKind::Strength, amount: -2,
        });
        assert_eq!(ds.statuses.strength, -2);
    }

    #[test]
    fn lifecycle_events_set_turn_energy_intent_outcome() {
        let mut ds = fixture();
        apply_event(&mut ds, &CombatEvent::TurnStarted { turn: 3 });
        apply_event(&mut ds, &CombatEvent::EnergySet { energy: 2 });
        apply_event(&mut ds, &CombatEvent::IntentSet {
            index: 0, intent: IntentKind::Attack { damage: 11, hits: 1 },
        });
        assert_eq!(ds.turn, 3);
        assert_eq!(ds.energy, 2);
        assert_eq!(ds.enemies[0].intent, Some(IntentKind::Attack { damage: 11, hits: 1 }));

        apply_event(&mut ds, &CombatEvent::EnemyDied { index: 0 });
        assert!(!ds.enemies[0].alive);
        assert_eq!(ds.enemies[0].intent, None);

        apply_event(&mut ds, &CombatEvent::Victory);
        assert_eq!(ds.outcome, Some(Outcome::Victory));
    }

    #[test]
    fn block_events_gain_and_reset() {
        let mut ds = fixture();
        apply_event(&mut ds, &CombatEvent::BlockGained { target: TargetRef::Player, amount: 5 });
        assert_eq!(ds.player_block, 5);
        apply_event(&mut ds, &CombatEvent::BlockReset { target: TargetRef::Player });
        assert_eq!(ds.player_block, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p helheim anim`
Expected: compile error (`DisplayState` undefined — the stub has none of this).

- [ ] **Step 3: Implement the module** — replace everything above the tests with:

```rust
use std::collections::VecDeque;

use bevy::prelude::*;
use helheim_core::cards::{CardId, CardKind};
use helheim_core::combat::{CombatEvent, IntentKind, Outcome, TargetRef};
use helheim_core::run::RunState;
use helheim_core::statuses::{StatusKind, Statuses};

use crate::theme::{self, UiFont};

pub const BEAT_SECONDS: f32 = 0.18;

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyView {
    pub name: &'static str,
    pub hp: u32,
    pub max_hp: u32,
    pub block: u32,
    pub statuses: Statuses,
    pub intent: Option<IntentKind>,
    pub alive: bool,
}

/// What the player currently SEES. Converges to the core state as the
/// event queue drains; equal to it when the queue is empty.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct DisplayState {
    pub player_hp: u32,
    pub player_max_hp: u32,
    pub player_block: u32,
    pub energy: u32,
    pub statuses: Statuses,
    pub hand: Vec<CardId>,
    pub draw_count: u32,
    pub discard_count: u32,
    pub enemies: Vec<EnemyView>,
    pub outcome: Option<Outcome>,
    pub turn: u32,
}

impl DisplayState {
    /// Pre-replay snapshot of a just-begun fight: enemies at full HP, empty
    /// hand, no energy — the opening events animate everything in.
    pub fn new_for(run: &RunState) -> Self {
        let c = run.combat.as_ref().expect("fight begun");
        DisplayState {
            player_hp: c.player.hp,
            player_max_hp: c.player.max_hp,
            player_block: 0,
            energy: 0,
            statuses: Statuses::default(),
            hand: Vec::new(),
            draw_count: (c.draw.len() + c.hand.len()) as u32,
            discard_count: 0,
            enemies: c
                .enemies
                .iter()
                .map(|e| EnemyView {
                    name: e.species.name(),
                    hp: e.max_hp,
                    max_hp: e.max_hp,
                    block: 0,
                    statuses: Statuses {
                        curl_up: e.statuses.curl_up,
                        ..Default::default()
                    },
                    intent: None,
                    alive: true,
                })
                .collect(),
            outcome: None,
            turn: 0,
        }
    }
}

/// Map one core event onto the display. Pure data; unit-tested below.
pub fn apply_event(ds: &mut DisplayState, ev: &CombatEvent) {
    match *ev {
        CombatEvent::TurnStarted { turn } => ds.turn = turn,
        CombatEvent::EnergySet { energy } => ds.energy = energy,
        CombatEvent::CardDrawn { card } => {
            ds.draw_count = ds.draw_count.saturating_sub(1);
            ds.hand.push(card);
        }
        CombatEvent::DeckShuffled => {
            ds.draw_count += ds.discard_count;
            ds.discard_count = 0;
        }
        CombatEvent::CardPlayed { card, hand_index } => {
            if hand_index < ds.hand.len() {
                ds.hand.remove(hand_index);
            }
            if card.spec().kind != CardKind::Power {
                ds.discard_count += 1;
            }
        }
        CombatEvent::CardAddedToDiscard { .. } => ds.discard_count += 1,
        CombatEvent::HandDiscarded => {
            ds.discard_count += ds.hand.len() as u32;
            ds.hand.clear();
        }
        CombatEvent::BlockReset { target } => match target {
            TargetRef::Player => ds.player_block = 0,
            TargetRef::Enemy(i) => {
                if let Some(e) = ds.enemies.get_mut(i) {
                    e.block = 0;
                }
            }
        },
        CombatEvent::BlockGained { target, amount } => match target {
            TargetRef::Player => ds.player_block += amount,
            TargetRef::Enemy(i) => {
                if let Some(e) = ds.enemies.get_mut(i) {
                    e.block += amount;
                }
            }
        },
        CombatEvent::DamageDealt { target, blocked, hp_lost, .. } => match target {
            TargetRef::Player => {
                ds.player_block = ds.player_block.saturating_sub(blocked);
                ds.player_hp = ds.player_hp.saturating_sub(hp_lost);
            }
            TargetRef::Enemy(i) => {
                if let Some(e) = ds.enemies.get_mut(i) {
                    e.block = e.block.saturating_sub(blocked);
                    e.hp = e.hp.saturating_sub(hp_lost);
                }
            }
        },
        CombatEvent::StatusApplied { target, status, amount } => {
            if let Some(s) = statuses_of(ds, target) {
                bump_status(s, status, amount);
            }
        }
        CombatEvent::StatusTicked { target, status, remaining } => {
            if let Some(s) = statuses_of(ds, target) {
                set_duration(s, status, remaining);
            }
        }
        CombatEvent::StatusExpired { target, status } => {
            if let Some(s) = statuses_of(ds, target) {
                clear_status(s, status);
            }
        }
        CombatEvent::EnemyMoved { .. } => {}
        CombatEvent::IntentSet { index, intent } => {
            if let Some(e) = ds.enemies.get_mut(index) {
                e.intent = Some(intent);
            }
        }
        CombatEvent::EnemyDied { index } => {
            if let Some(e) = ds.enemies.get_mut(index) {
                e.alive = false;
                e.intent = None;
            }
        }
        CombatEvent::PlayerDied => ds.outcome = Some(Outcome::Defeat),
        CombatEvent::Victory => ds.outcome = Some(Outcome::Victory),
    }
}

fn statuses_of(ds: &mut DisplayState, target: TargetRef) -> Option<&mut Statuses> {
    match target {
        TargetRef::Player => Some(&mut ds.statuses),
        TargetRef::Enemy(i) => ds.enemies.get_mut(i).map(|e| &mut e.statuses),
    }
}

fn bump_status(s: &mut Statuses, kind: StatusKind, amount: i32) {
    match kind {
        StatusKind::Strength => s.strength += amount,
        StatusKind::Vulnerable => s.vulnerable += amount.max(0) as u32,
        StatusKind::Weak => s.weak += amount.max(0) as u32,
        StatusKind::Ritual => s.ritual += amount.max(0) as u32,
        StatusKind::Enrage => s.enrage += amount.max(0) as u32,
        StatusKind::CurlUp => s.curl_up = Some(amount.max(0) as u32),
        StatusKind::StrengthDown => s.strength_down += amount.max(0) as u32,
    }
}

fn set_duration(s: &mut Statuses, kind: StatusKind, remaining: u32) {
    match kind {
        StatusKind::Vulnerable => s.vulnerable = remaining,
        StatusKind::Weak => s.weak = remaining,
        _ => {}
    }
}

fn clear_status(s: &mut Statuses, kind: StatusKind) {
    match kind {
        StatusKind::Strength => s.strength = 0,
        StatusKind::Vulnerable => s.vulnerable = 0,
        StatusKind::Weak => s.weak = 0,
        StatusKind::Ritual => s.ritual = 0,
        StatusKind::Enrage => s.enrage = 0,
        StatusKind::CurlUp => s.curl_up = None,
        StatusKind::StrengthDown => s.strength_down = 0,
    }
}

// ---------- queue, beats, floaters ----------

#[derive(Resource, Default)]
pub struct EventQueue(pub VecDeque<CombatEvent>);

#[derive(Resource)]
pub struct BeatTimer(pub Timer);

/// Marks a UI panel as the visual home of a combatant (floaters spawn here).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct PanelTarget(pub TargetRef);

#[derive(Component)]
struct Floater {
    timer: Timer,
}

/// run_if condition: player input is allowed only when nothing is animating.
pub fn queue_empty(queue: Res<EventQueue>) -> bool {
    queue.0.is_empty()
}

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventQueue>()
            .insert_resource(BeatTimer(Timer::from_seconds(
                BEAT_SECONDS,
                TimerMode::Repeating,
            )))
            .add_systems(
                Update,
                (
                    drain_queue.run_if(resource_exists::<DisplayState>),
                    float_floaters,
                ),
            );
    }
}

/// Which events pause the replay for a visible beat, and what they show.
fn beat_visual(ev: &CombatEvent) -> Option<(TargetRef, String, Color)> {
    match ev {
        CombatEvent::DamageDealt { target, hp_lost, .. } => Some((
            *target,
            if *hp_lost > 0 { format!("-{hp_lost}") } else { "Blocked".into() },
            if *hp_lost > 0 { theme::HP_COLOR } else { theme::TEXT_DIM },
        )),
        CombatEvent::BlockGained { target, amount } => {
            Some((*target, format!("+{amount} Block"), theme::BLOCK_COLOR))
        }
        CombatEvent::EnemyMoved { index, mv } => Some((
            TargetRef::Enemy(*index),
            mv.name().to_string(),
            theme::TEXT,
        )),
        CombatEvent::StatusApplied { target, status, amount } => {
            Some((*target, format!("{status:?} {amount:+}"), theme::TEXT_DIM))
        }
        _ => None,
    }
}

fn is_beat(ev: &CombatEvent) -> bool {
    beat_visual(ev).is_some()
        || matches!(ev, CombatEvent::Victory | CombatEvent::PlayerDied)
}

/// Pop events each beat: bookkeeping applies instantly, beat events pause.
fn drain_queue(
    time: Res<Time>,
    mut timer: ResMut<BeatTimer>,
    mut queue: ResMut<EventQueue>,
    mut ds: ResMut<DisplayState>,
    mut commands: Commands,
    font: Res<UiFont>,
    panels: Query<(Entity, &PanelTarget)>,
) {
    timer.0.tick(time.delta());
    if queue.0.is_empty() || !timer.0.just_finished() {
        return;
    }
    while let Some(ev) = queue.0.pop_front() {
        let visual = beat_visual(&ev);
        let beat = is_beat(&ev);
        apply_event(&mut ds, &ev);
        if let Some((target, text, color)) = visual {
            if let Some((panel, _)) = panels.iter().find(|(_, p)| p.0 == target) {
                spawn_floater(&mut commands, &font, panel, text, color);
            }
        }
        if beat {
            break;
        }
    }
}

fn spawn_floater(
    commands: &mut Commands,
    font: &UiFont,
    parent: Entity,
    label: String,
    color: Color,
) {
    let floater = commands
        .spawn((
            Floater {
                timer: Timer::from_seconds(0.7, TimerMode::Once),
            },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(-12.),
                right: Val::Px(6.),
                ..default()
            },
            theme::text(font, label, 24., color),
            ZIndex(10),
        ))
        .id();
    commands.entity(parent).add_child(floater);
}

fn float_floaters(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Floater, &mut Node, &mut TextColor)>,
) {
    for (e, mut f, mut node, mut color) in &mut q {
        f.timer.tick(time.delta());
        let t = f.timer.fraction();
        node.top = Val::Px(-12. - 34. * t);
        color.0 = color.0.with_alpha(1.0 - t);
        if f.timer.finished() {
            commands.entity(e).despawn();
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p helheim anim`
Expected: 6 passed (plus the smoke test still green via `cargo test -p helheim`).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(shell): DisplayState event replay with beat pacing and floaters"
```
### Task 18: Combat screen — layout, bindings, input, targeting

**Files:**
- Modify: `crates/helheim/src/screens/combat.rs` (replace the stub entirely)

Verified by playing; the logic this screen calls is already tested underneath.

- [ ] **Step 1: Write the module skeleton, components, and plugin wiring**

```rust
use bevy::prelude::*;
use helheim_core::cards::{CardId, Targeting};
use helheim_core::combat::{Action, IntentKind, TargetRef};
use helheim_core::run::Stage;
use helheim_core::statuses::Statuses;

use crate::anim::{queue_empty, DisplayState, EventQueue, PanelTarget};
use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub struct CombatScreenPlugin;

impl Plugin for CombatScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingCard>()
            .init_resource::<TargetCursor>()
            .add_systems(OnEnter(AppState::Combat), enter_combat)
            .add_systems(OnExit(AppState::Combat), exit_combat)
            .add_systems(
                Update,
                (
                    (card_click, enemy_click, end_turn_button, keyboard)
                        .run_if(in_state(AppState::Combat))
                        .run_if(queue_empty),
                    (sync_texts, rebuild_hand, highlight_enemies, post_combat)
                        .run_if(in_state(AppState::Combat)),
                ),
            );
    }
}

/// Hand index of a single-target card waiting for the player to pick an enemy.
#[derive(Resource, Default)]
struct PendingCard(Option<usize>);

/// Keyboard target cursor (index into ds.enemies) while a card is pending.
#[derive(Resource, Default)]
struct TargetCursor(usize);

#[derive(Component)]
struct CombatRoot;

/// One text label bound to one piece of DisplayState.
#[derive(Component)]
enum Bind {
    Turn,
    Piles,
    Energy,
    Hp(TargetRef),
    Block(TargetRef),
    Status(TargetRef),
    Intent(usize),
}

#[derive(Component)]
struct HandRow;

#[derive(Component)]
struct CardButton(usize);

#[derive(Component)]
struct EndTurnButton;
```

- [ ] **Step 2: Implement enter/exit and the UI tree**

```rust
fn enter_combat(
    mut commands: Commands,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    font: Res<UiFont>,
) {
    if session.run.combat.is_none() {
        let events = session
            .run
            .begin_fight()
            .expect("entered Combat outside a Fight stage");
        let ds = DisplayState::new_for(&session.run);
        queue.0.clear();
        queue.0.extend(events);
        spawn_combat_ui(&mut commands, &font, &ds);
        commands.insert_resource(ds);
    }
}

fn exit_combat(
    mut commands: Commands,
    mut queue: ResMut<EventQueue>,
    mut pending: ResMut<PendingCard>,
    roots: Query<Entity, With<CombatRoot>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    queue.0.clear();
    pending.0 = None;
    commands.remove_resource::<DisplayState>();
}

fn spawn_combat_ui(commands: &mut Commands, font: &UiFont, ds: &DisplayState) {
    commands
        .spawn((
            CombatRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| {
            // ---- top bar ----
            root.spawn(Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(12.)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|bar| {
                bar.spawn((Bind::Turn, theme::text(font, "", 20., theme::TEXT_DIM)));
                bar.spawn((Bind::Piles, theme::text(font, "", 20., theme::TEXT_DIM)));
            });

            // ---- battlefield ----
            root.spawn(Node {
                width: Val::Percent(100.),
                flex_grow: 1.,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(24.)),
                column_gap: Val::Px(40.),
                ..default()
            })
            .with_children(|field| {
                // player panel
                field
                    .spawn((
                        PanelTarget(TargetRef::Player),
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(16.)),
                            row_gap: Val::Px(6.),
                            min_width: Val::Px(220.),
                            ..default()
                        },
                        BackgroundColor(theme::PANEL),
                    ))
                    .with_children(|p| {
                        p.spawn(theme::text(font, "The Berserker", 24., theme::ACCENT));
                        p.spawn((Bind::Hp(TargetRef::Player), theme::text(font, "", 22., theme::HP_COLOR)));
                        p.spawn((Bind::Block(TargetRef::Player), theme::text(font, "", 18., theme::BLOCK_COLOR)));
                        p.spawn((Bind::Status(TargetRef::Player), theme::text(font, "", 16., theme::TEXT_DIM)));
                    });

                // enemies, in spawn order
                field
                    .spawn(Node {
                        flex_grow: 1.,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: Val::Px(24.),
                        ..default()
                    })
                    .with_children(|row| {
                        for (i, enemy) in ds.enemies.iter().enumerate() {
                            row.spawn((
                                PanelTarget(TargetRef::Enemy(i)),
                                Button, // clickable for targeting
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    padding: UiRect::all(Val::Px(16.)),
                                    row_gap: Val::Px(6.),
                                    min_width: Val::Px(200.),
                                    ..default()
                                },
                                BackgroundColor(theme::PANEL),
                            ))
                            .with_children(|p| {
                                p.spawn(theme::text(font, enemy.name, 22., theme::TEXT));
                                p.spawn((Bind::Intent(i), theme::text(font, "", 18., theme::ENERGY_COLOR)));
                                p.spawn((Bind::Hp(TargetRef::Enemy(i)), theme::text(font, "", 20., theme::HP_COLOR)));
                                p.spawn((Bind::Block(TargetRef::Enemy(i)), theme::text(font, "", 16., theme::BLOCK_COLOR)));
                                p.spawn((Bind::Status(TargetRef::Enemy(i)), theme::text(font, "", 14., theme::TEXT_DIM)));
                            });
                        }
                    });
            });

            // ---- bottom bar: energy, hand, end turn ----
            root.spawn(Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(12.)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.),
                ..default()
            })
            .with_children(|bar| {
                bar.spawn((Bind::Energy, theme::text(font, "", 30., theme::ENERGY_COLOR)));
                bar.spawn((
                    HandRow,
                    Node {
                        flex_grow: 1.,
                        column_gap: Val::Px(10.),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ));
                theme::button(bar, font, EndTurnButton, "End Turn [E]");
            });
        });
}
```

- [ ] **Step 3: Implement the sync, hand-rebuild, and highlight systems**

```rust
fn status_line(s: &Statuses) -> String {
    let mut parts = Vec::new();
    if s.strength != 0 { parts.push(format!("Str {:+}", s.strength)); }
    if s.vulnerable > 0 { parts.push(format!("Vuln {}", s.vulnerable)); }
    if s.weak > 0 { parts.push(format!("Weak {}", s.weak)); }
    if s.ritual > 0 { parts.push(format!("Ritual {}", s.ritual)); }
    if s.enrage > 0 { parts.push(format!("Enrage {}", s.enrage)); }
    if let Some(c) = s.curl_up { parts.push(format!("Curl Up {c}")); }
    if s.strength_down > 0 { parts.push(format!("Str Down {}", s.strength_down)); }
    parts.join("  ")
}

fn intent_line(intent: Option<IntentKind>) -> String {
    match intent {
        Some(IntentKind::Attack { damage, hits: 1 }) => format!("Intent: ATK {damage}"),
        Some(IntentKind::Attack { damage, hits }) => format!("Intent: ATK {damage}x{hits}"),
        Some(IntentKind::AttackDefend { damage }) => format!("Intent: ATK {damage} +DEF"),
        Some(IntentKind::Defend) => "Intent: DEFEND".into(),
        Some(IntentKind::Buff) => "Intent: BUFF".into(),
        Some(IntentKind::Debuff) => "Intent: DEBUFF".into(),
        None => String::new(),
    }
}

fn sync_texts(ds: Res<DisplayState>, mut q: Query<(&Bind, &mut Text)>) {
    if !ds.is_changed() {
        return;
    }
    for (bind, mut text) in &mut q {
        text.0 = match bind {
            Bind::Turn => format!("Turn {}", ds.turn),
            Bind::Piles => format!("Draw {}   Discard {}", ds.draw_count, ds.discard_count),
            Bind::Energy => format!("Energy {}/3", ds.energy),
            Bind::Hp(TargetRef::Player) => format!("HP {}/{}", ds.player_hp, ds.player_max_hp),
            Bind::Hp(TargetRef::Enemy(i)) => match ds.enemies.get(*i) {
                Some(e) if e.alive => format!("HP {}/{}", e.hp, e.max_hp),
                _ => "DEAD".into(),
            },
            Bind::Block(TargetRef::Player) => block_line(ds.player_block),
            Bind::Block(TargetRef::Enemy(i)) => {
                block_line(ds.enemies.get(*i).map(|e| e.block).unwrap_or(0))
            }
            Bind::Status(TargetRef::Player) => status_line(&ds.statuses),
            Bind::Status(TargetRef::Enemy(i)) => {
                ds.enemies.get(*i).map(|e| status_line(&e.statuses)).unwrap_or_default()
            }
            Bind::Intent(i) => {
                ds.enemies.get(*i).map(|e| intent_line(e.intent)).unwrap_or_default()
            }
        };
    }
}

fn block_line(block: u32) -> String {
    if block > 0 { format!("Block {block}") } else { String::new() }
}

fn rebuild_hand(
    mut commands: Commands,
    ds: Res<DisplayState>,
    font: Res<UiFont>,
    row: Query<Entity, With<HandRow>>,
    existing: Query<Entity, With<CardButton>>,
) {
    if !ds.is_changed() {
        return;
    }
    let Ok(row) = row.single() else { return };
    for e in &existing {
        commands.entity(e).despawn();
    }
    for (i, card) in ds.hand.iter().enumerate() {
        let spec = card.spec();
        let affordable = spec.cost <= ds.energy;
        let bg = if affordable { theme::PANEL } else { theme::PANEL_DIM };
        let label = if i < 9 { format!("[{}]", i + 1) } else { "[0]".into() };
        let button = commands
            .spawn((
                CardButton(i),
                Button,
                Node {
                    width: Val::Px(150.),
                    height: Val::Px(170.),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(10.)),
                    ..default()
                },
                BackgroundColor(bg),
            ))
            .with_children(|c| {
                c.spawn(theme::text(&font, format!("({}) {}", spec.cost, spec.name), 17., theme::TEXT));
                c.spawn(theme::text(&font, spec.text, 14., theme::TEXT_DIM));
                c.spawn(theme::text(&font, label, 13., theme::TEXT_DIM));
            })
            .id();
        commands.entity(row).add_child(button);
    }
}

/// Highlight valid targets while a card is pending (and the keyboard cursor).
fn highlight_enemies(
    ds: Res<DisplayState>,
    pending: Res<PendingCard>,
    cursor: Res<TargetCursor>,
    mut panels: Query<(&PanelTarget, &mut BackgroundColor, Option<&Interaction>)>,
) {
    for (panel, mut bg, interaction) in &mut panels {
        let TargetRef::Enemy(i) = panel.0 else { continue };
        let alive = ds.enemies.get(i).map(|e| e.alive).unwrap_or(false);
        let targeting = pending.0.is_some() && alive;
        let hovered = matches!(interaction, Some(Interaction::Hovered));
        let cursor_here = targeting && cursor.0 == i;
        *bg = BackgroundColor(if targeting && (hovered || cursor_here) {
            theme::PANEL_HOVER
        } else if targeting {
            theme::ACCENT.with_alpha(0.25)
        } else {
            theme::PANEL
        });
    }
}
```

- [ ] **Step 4: Implement input dispatch and the post-combat transition**

```rust
fn dispatch(action: Action, session: &mut Session, queue: &mut EventQueue) {
    match session.run.apply(action) {
        Ok(events) => queue.0.extend(events),
        // The UI should have prevented this; the core stayed consistent.
        Err(err) => warn!("rejected action {action:?}: {err:?}"),
    }
}

/// Click (or hotkey) a card: dispatch immediately, or arm targeting mode.
fn try_play(
    index: usize,
    ds: &DisplayState,
    pending: &mut PendingCard,
    cursor: &mut TargetCursor,
    session: &mut Session,
    queue: &mut EventQueue,
) {
    let Some(card) = ds.hand.get(index) else { return };
    let spec = card.spec();
    if spec.cost > ds.energy {
        return;
    }
    let living: Vec<usize> = ds
        .enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive)
        .map(|(i, _)| i)
        .collect();
    match spec.targeting {
        Targeting::SingleEnemy if living.len() > 1 => {
            pending.0 = Some(index);
            cursor.0 = living[0];
        }
        _ => dispatch(
            Action::PlayCard { hand_index: index, target: None },
            session,
            queue,
        ),
    }
}

fn card_click(
    ds: Res<DisplayState>,
    mut pending: ResMut<PendingCard>,
    mut cursor: ResMut<TargetCursor>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    buttons: Query<(&Interaction, &CardButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            try_play(button.0, &ds, &mut pending, &mut cursor, &mut session, &mut queue);
        }
    }
}

fn enemy_click(
    ds: Res<DisplayState>,
    mut pending: ResMut<PendingCard>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    panels: Query<(&Interaction, &PanelTarget), Changed<Interaction>>,
) {
    let Some(card_index) = pending.0 else { return };
    for (interaction, panel) in &panels {
        let TargetRef::Enemy(i) = panel.0 else { continue };
        let alive = ds.enemies.get(i).map(|e| e.alive).unwrap_or(false);
        if *interaction == Interaction::Pressed && alive {
            pending.0 = None;
            dispatch(
                Action::PlayCard { hand_index: card_index, target: Some(i) },
                &mut session,
                &mut queue,
            );
            return;
        }
    }
}

fn end_turn_button(
    mut pending: ResMut<PendingCard>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<EndTurnButton>)>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            pending.0 = None;
            dispatch(Action::EndTurn, &mut session, &mut queue);
        }
    }
}

const DIGIT_KEYS: [KeyCode; 10] = [
    KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5,
    KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9, KeyCode::Digit0,
];

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    ds: Res<DisplayState>,
    mut pending: ResMut<PendingCard>,
    mut cursor: ResMut<TargetCursor>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        pending.0 = None;
        return;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        pending.0 = None;
        dispatch(Action::EndTurn, &mut session, &mut queue);
        return;
    }
    if let Some(card_index) = pending.0 {
        // Targeting mode: cycle living enemies, Enter to confirm.
        let living: Vec<usize> = ds
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.alive)
            .map(|(i, _)| i)
            .collect();
        if living.is_empty() {
            pending.0 = None;
            return;
        }
        let pos = living.iter().position(|&i| i == cursor.0).unwrap_or(0);
        if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::ArrowRight) {
            cursor.0 = living[(pos + 1) % living.len()];
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            cursor.0 = living[(pos + living.len() - 1) % living.len()];
        }
        if keys.just_pressed(KeyCode::Enter) {
            pending.0 = None;
            dispatch(
                Action::PlayCard { hand_index: card_index, target: Some(cursor.0) },
                &mut session,
                &mut queue,
            );
        }
        return;
    }
    for (n, key) in DIGIT_KEYS.iter().enumerate() {
        if keys.just_pressed(*key) {
            try_play(n, &ds, &mut pending, &mut cursor, &mut session, &mut queue);
        }
    }
}

/// When the fight's outcome has fully animated, follow the run's stage.
fn post_combat(
    ds: Res<DisplayState>,
    queue: Res<EventQueue>,
    session: Res<Session>,
    mut next: ResMut<NextState<AppState>>,
) {
    if ds.outcome.is_none() || !queue.0.is_empty() {
        return;
    }
    match session.run.stage {
        Stage::Reward { .. } => next.set(AppState::Reward),
        Stage::Victory => next.set(AppState::Victory),
        Stage::Defeat => next.set(AppState::GameOver),
        Stage::Fight(_) => {}
    }
}
```

- [ ] **Step 5: Build and play-test fight 1**

```bash
cargo run -p helheim --features dev
```

Expected: menu → combat vs a Draugr Chanter or Grave Wolf. Cards show in hand
with cost/name/text and hotkey labels; clicking an attack with one enemy
auto-plays it; floaters show damage and moves; End Turn animates the enemy
turn beat by beat; input is dead while animating; energy and piles update;
unaffordable cards are dimmed. (Reward/Victory/GameOver still stubs: after a
won fight the screen will go blank — that's Task 19.)

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(shell): combat screen with bindings, mouse/keyboard input, targeting"
```

### Task 19: Reward & end screens, README, final verification

**Files:**
- Modify: `crates/helheim/src/screens/reward.rs` (replace stub)
- Modify: `crates/helheim/src/screens/end.rs` (replace stub)
- Create: `README.md`

- [ ] **Step 1: Implement the reward screen**

```rust
// screens/reward.rs
use bevy::prelude::*;
use helheim_core::run::Stage;

use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub struct RewardPlugin;

impl Plugin for RewardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Reward), spawn_reward)
            .add_systems(OnExit(AppState::Reward), despawn_reward)
            .add_systems(Update, reward_clicks.run_if(in_state(AppState::Reward)));
    }
}

#[derive(Component)]
struct RewardRoot;

#[derive(Component)]
struct RewardButton(usize);

#[derive(Component)]
struct SkipButton;

fn spawn_reward(mut commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    let Stage::Reward { offer, .. } = session.run.stage else { return };
    commands
        .spawn((
            RewardRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(30.),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(theme::text(&font, "Claim your spoils", 40., theme::ACCENT));
            root.spawn(Node {
                column_gap: Val::Px(20.),
                ..default()
            })
            .with_children(|row| {
                for (i, card) in offer.iter().enumerate() {
                    let spec = card.spec();
                    row.spawn((
                        RewardButton(i),
                        Button,
                        Node {
                            width: Val::Px(190.),
                            height: Val::Px(230.),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(14.)),
                            ..default()
                        },
                        BackgroundColor(theme::PANEL),
                    ))
                    .with_children(|c| {
                        c.spawn(theme::text(&font, format!("({}) {}", spec.cost, spec.name), 20., theme::TEXT));
                        c.spawn(theme::text(&font, spec.text, 16., theme::TEXT_DIM));
                    });
                }
            });
            theme::button(root, &font, SkipButton, "Walk on (skip)");
        });
}

fn despawn_reward(mut commands: Commands, q: Query<Entity, With<RewardRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn reward_clicks(
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<AppState>>,
    cards: Query<(&Interaction, &RewardButton), Changed<Interaction>>,
    skips: Query<&Interaction, (Changed<Interaction>, With<SkipButton>)>,
) {
    for (interaction, button) in &cards {
        if *interaction == Interaction::Pressed
            && session.run.choose_reward(Some(button.0)).is_ok()
        {
            next.set(AppState::Combat);
            return;
        }
    }
    for interaction in &skips {
        if *interaction == Interaction::Pressed && session.run.choose_reward(None).is_ok() {
            next.set(AppState::Combat);
            return;
        }
    }
}
```

- [ ] **Step 2: Implement the end screens**

```rust
// screens/end.rs
use bevy::prelude::*;
use helheim_core::run::RunState;

use crate::theme::{self, UiFont};
use crate::{AppState, CliSeed, Session};

pub struct EndScreensPlugin;

impl Plugin for EndScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Victory), spawn_victory)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over)
            .add_systems(OnExit(AppState::Victory), despawn_end)
            .add_systems(OnExit(AppState::GameOver), despawn_end)
            .add_systems(
                Update,
                end_clicks.run_if(in_state(AppState::Victory).or(in_state(AppState::GameOver))),
            );
    }
}

#[derive(Component)]
struct EndRoot;

#[derive(Component)]
struct AgainButton;

#[derive(Component)]
struct MenuButton;

fn spawn_victory(commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    spawn_end(commands, &session, &font, "THE BARROW ROAD IS CLEARED", theme::ENERGY_COLOR);
}

fn spawn_game_over(commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    spawn_end(commands, &session, &font, "SLAIN ON THE BARROW ROAD", theme::ACCENT);
}

fn spawn_end(
    mut commands: Commands,
    session: &Session,
    font: &UiFont,
    title: &str,
    title_color: Color,
) {
    let run = &session.run;
    let stats = [
        format!("Turns taken: {}", run.stats.turns),
        format!("Damage dealt: {}", run.stats.damage_dealt),
        format!("Damage taken: {}", run.stats.damage_taken),
        format!("Final deck: {} cards", run.master_deck.len()),
        format!("Seed: {}", run.seed),
    ];
    commands
        .spawn((
            EndRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(theme::text(font, title, 52., title_color));
            for line in stats {
                root.spawn(theme::text(font, line, 22., theme::TEXT));
            }
            root.spawn(Node {
                column_gap: Val::Px(16.),
                margin: UiRect::top(Val::Px(20.)),
                ..default()
            })
            .with_children(|row| {
                theme::button(row, font, AgainButton, "Descend Again");
                theme::button(row, font, MenuButton, "Back to Menu");
            });
        });
}

fn despawn_end(mut commands: Commands, q: Query<Entity, With<EndRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn end_clicks(
    mut commands: Commands,
    cli: Res<CliSeed>,
    mut next: ResMut<NextState<AppState>>,
    again: Query<&Interaction, (Changed<Interaction>, With<AgainButton>)>,
    menu: Query<&Interaction, (Changed<Interaction>, With<MenuButton>)>,
) {
    for interaction in &again {
        if *interaction == Interaction::Pressed {
            commands.insert_resource(Session { run: RunState::new(cli.next_seed()) });
            next.set(AppState::Combat);
            return;
        }
    }
    for interaction in &menu {
        if *interaction == Interaction::Pressed {
            next.set(AppState::Menu);
            return;
        }
    }
}
```

(`in_state(..).or(..)`: if this combinator name drifted in your Bevy version,
register `end_clicks` twice, once per state, instead.)

- [ ] **Step 3: Write `README.md`**

```markdown
# Helheim

A Slay-the-Spire-style roguelike deck-builder set on the Norse barrow road,
written in Rust. The rules engine (`helheim_core`) is pure, deterministic,
and fully unit-tested; the presentation is Bevy.

## Run

    cargo run -p helheim                 # release-ish dev build
    cargo run -p helheim --features dev  # fast-rebuild dev loop
    cargo run -p helheim -- --seed 7     # reproducible run

## Controls

- Click a card to play it; click an enemy when a target is needed (Esc cancels)
- `1`–`9`/`0` play cards, Tab/arrows cycle targets, Enter confirms
- `E` or the button ends the turn

## Test

    cargo test --workspace               # core rules + shell unit tests
    cargo clippy --workspace --all-targets -- -D warnings

## Layout

- `crates/helheim_core` — cards, combat engine, enemy AI, run state (no Bevy)
- `crates/helheim` — Bevy shell: screens, animation queue, theme
- `docs/superpowers/specs/` — design specs; `docs/superpowers/plans/` — build plans

Phase 1 ships the 3-fight Barrow Road gauntlet. The roadmap (map, relics,
acts 2–3…) lives in the Phase 1 spec.

Font: Fira Sans (SIL Open Font License).
```

- [ ] **Step 4: Full verification gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all green. Then play through the manual checklist:

1. `cargo run -p helheim` → menu; Begin → fight 1 vs Chanter **or** Wolf.
2. Mouse path: play attacks/skills, watch floaters and intent numbers.
3. Keyboard path: digits, `E`; vs two rats — targeting with Tab/Enter and Esc.
4. Win fight 1 → reward screen shows 3 distinct cards; pick one → fight 2
   (pair); skip works too.
5. Fight 3 Troll: play a Skill, watch its attack intent rise (Enrage).
6. Win → Victory screen with stats + seed. Lose on purpose (end turns) →
   GameOver screen with seed. "Descend Again" and "Back to Menu" both work.
7. `cargo run -p helheim -- --seed 7` twice → same fight-1 species and same
   opening hand both times.

- [ ] **Step 5: Final commit**

```bash
git add -A && git commit -m "feat(shell): reward and end screens, README; Phase 1 complete"
```

---

## Definition of done (mirrors the spec)

1. `cargo run -p helheim` opens a window; a full gauntlet run is playable to
   victory and to death with mouse or keyboard.
2. All core tests pass; clippy is clean (`-D warnings`); fmt applied.
3. `--seed` reproduces a run deterministically; the seed shows on end screens.
4. The full-gauntlet integration test (`tests/gauntlet.rs`) exists and passes.
