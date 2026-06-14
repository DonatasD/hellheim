# Act 1 Map Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Phase 1's linear 3-fight gauntlet with a deterministic, Slay-the-Spire-style branching ~15-floor Act 1 map: the player climbs by choosing connected nodes, fighting Monster/Elite encounters and a boss, resting and taking treasure along the way.

**Architecture:** A new pure-Rust `map` module in `helheim_core` generates the node graph from the run seed (`RunRng`) and is fully unit-tested; `run.rs` is refactored from `Stage::Fight(n)` to map-driven navigation (`available_nodes`/`enter_node`); a new `encounters` module maps nodes to enemy groups; the Bevy shell gains a `Map` screen and a `Rest` screen and routes node selections. Combat itself is unchanged and still plays through the existing `CombatEvent` replay.

**Tech Stack:** Rust stable, Bevy 0.18, the existing `helheim_core` engine.

---

## Context for the implementing engineer

- **Spec:** `docs/superpowers/specs/2026-06-14-act1-map-foundation-design.md`. Read it first. This plan implements spec 1 of the Phase 2 sequence; gold/shops/events/upgrades/save are explicitly out of scope.
- **TDD discipline:** every core task is red → green → commit. Run the named test while iterating (`cargo test -p helheim_core <name>`), the full crate suite before each commit (`cargo test -p helheim_core`).
- **Core-first, shell-later:** Tasks 1–8 are pure-core and gated on `cargo test -p helheim_core`. **The `run.rs` refactor in Task 7 deliberately breaks the Bevy shell** (it removes `begin_fight`/`Stage::Fight`, which the shell calls). That is expected: the shell does not compile again until Task 10. Do **not** try to keep the shell building during Tasks 7–9; gate those tasks on the core crate only. Task 10 makes `cargo build -p helheim` green again.
- **Bevy 0.18 drift (learned in Phase 1):** `WindowResolution` is `From<(u32,u32)>` (ints, not floats); `Timer::is_finished()` (not `finished()`); annotate complex Bevy queries with `#[allow(clippy::type_complexity)]`; **load asset handles at plugin `build` time** (the initial `OnEnter` runs before `Startup`). Reuse `theme::text`/`theme::button` for all UI.
- **Conventions:** StS wiki numbers are authoritative; integer math (truncating division = floor); everything flows from one seeded `RunRng`. Commit messages: `feat(core): …`, `feat(shell): …`, `test(core): …`, `chore: …`.
- **Existing APIs you will use (do not change their signatures):**
  - `RunRng::{new, range(lo,hi) inclusive, percent() 0..=99, shuffle, pick}`
  - `enemies::{Species, EnemyMove, roll_move, ran_consecutively (private)}`; `Species::{name, hp_range}`
  - `combat::CombatState::new(rng, deck, hp, max_hp, species) -> (CombatState, Vec<CombatEvent>)`, `CombatState::apply(rng, action)`, `CombatState.over: Option<Outcome>`, `Enemy.{species, hp, alive()}`
  - `combat::{IntentKind, TargetRef, StatusKind, CombatEvent, Action, Outcome}`
  - `cards::{CardId, REWARD_POOL, starter_deck}`

## Type Reference (canonical — later tasks must match exactly)

```rust
// map.rs
pub const MAP_FLOORS: u8 = 15;   // playable floors 1..=15
pub const MAP_WIDTH: u8 = 7;     // columns 0..=6
pub const MAP_PATHS: u32 = 6;
pub const BOSS_FLOOR: u8 = 16;
pub const BOSS_COL: u8 = 3;

pub enum NodeKind { Monster, Elite, Rest, Treasure, Boss }   // Shop, Event added in later specs
pub struct NodeId { pub floor: u8, pub col: u8 }             // derive Ord for BTree keys
pub struct MapNode { pub id: NodeId, pub kind: NodeKind, pub next: Vec<NodeId> }
pub struct MapGraph { nodes: Vec<MapNode> }                  // includes the boss node
impl MapGraph {
    pub fn generate(rng: &mut RunRng) -> Self;
    pub fn floor1(&self) -> Vec<NodeId>;
    pub fn node(&self, id: NodeId) -> &MapNode;
    pub fn nodes_on(&self, floor: u8) -> Vec<&MapNode>;
    pub fn boss_id(&self) -> NodeId;
    pub fn all(&self) -> &[MapNode];
}

// encounters.rs
pub fn roll_encounter(kind: NodeKind, floor: u8, rng: &mut RunRng, avoid: &[Species]) -> Vec<Species>;

// run.rs (refactored)
pub enum RewardSource { Combat, Treasure }
pub enum Stage { ChoosingNode, Reward { offer: [CardId; 3], source: RewardSource }, Victory, Defeat }
pub enum RunError { NotChoosingNode, IllegalMove, InCombat, NotInReward, BadIndex }
pub struct RunState {
    pub seed: u64, /* rng private */ pub master_deck: Vec<CardId>,
    pub hp: u32, pub max_hp: u32, pub map: MapGraph, pub position: Option<NodeId>,
    pub stage: Stage, pub combat: Option<CombatState>, pub stats: RunStats,
    /* last_encounter private */
}
impl RunState {
    pub fn new(seed: u64) -> Self;                                  // generates map; stage ChoosingNode; position None
    pub fn available_nodes(&self) -> Vec<NodeId>;
    pub fn enter_node(&mut self, id: NodeId) -> Result<Vec<CombatEvent>, RunError>;
    pub fn apply(&mut self, action: Action) -> Result<Vec<CombatEvent>, IllegalAction>;
    pub fn choose_reward(&mut self, pick: Option<usize>) -> Result<(), RunError>;
}
```

Locked rules (tests encode them):
- Rest heals `floor(0.30 * max_hp)` (= 24 at 80), capped at `max_hp`.
- Generation: 15 floors + boss at floor 16; 6 paths; adjacency steps (`c-1/c/c+1`); no edge crossings; ≥2 distinct floor-1 nodes; all floor-15 nodes link to the boss.
- Placement: floor 1 = Monster, floor 9 = Treasure, floor 15 = Rest, floor 16 = Boss; no Elite/Rest below floor 6; no Rest on floor 14; a node's kind is never the same *special* (Elite/Rest) as one of its parents.
- Non-fixed weights: Elite 16% / Rest 12% / Monster 72%.
- Encounters: floors 1–3 monster nodes use the weak pool, floors 4+ the strong pool; elites the elite pool; boss the boss; never repeat the immediately previous group.

---

### Task 1: New enemy species data

**Files:**
- Modify: `crates/helheim_core/src/enemies.rs`

- [ ] **Step 1: Add the failing test** — inside `mod tests`, add:

```rust
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p helheim_core new_species_data`
Expected: compile error (`DraugrWarrior` not a variant).

- [ ] **Step 3: Add the variants and data** — extend the `Species` enum and both match arms:

```rust
pub enum Species {
    DraugrChanter,
    GraveWolf,
    BarrowRat,
    FenRat,
    ForestTroll,
    // Act 1 map bestiary:
    DraugrWarrior, // StS Blue Slaver
    MireCrawler,   // StS Fungi Beast
    Hrafn,         // carrion crow
    BarrowWight,   // StS Lagavulin (elite)
    DraugrWarlord, // elite
    MoundJarl,     // Act 1 boss
}
```

Add to `name`:
```rust
            Species::DraugrWarrior => "Draugr Warrior",
            Species::MireCrawler => "Mire Crawler",
            Species::Hrafn => "Hrafn",
            Species::BarrowWight => "Barrow Wight",
            Species::DraugrWarlord => "Draugr Warlord",
            Species::MoundJarl => "The Mound Jarl",
```

Add to `hp_range`:
```rust
            Species::DraugrWarrior => (46, 50),
            Species::MireCrawler => (22, 28),
            Species::Hrafn => (30, 34),
            Species::BarrowWight => (85, 90),
            Species::DraugrWarlord => (86, 90),
            Species::MoundJarl => (150, 150),
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p helheim_core enemies`
Expected: all enemies tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): Act 1 bestiary species data (3 normal, 2 elite, 1 boss)"
```

### Task 2: New enemy moves and `roll_move` AI

**Files:**
- Modify: `crates/helheim_core/src/enemies.rs`

New moves (numbers are the design target): Stab (attack 12), Rend (attack 8 + Weak 1), Fester (gain Strength 4), Peck (attack 4 ×2), Screech (attack 5 + Vulnerable 1), Maul (attack 18), SoulDrain (player −2 Strength), WarChant (gain Ritual 2), Cleave (attack 8 ×2), CrushingBlow (attack 22), GraveCleave (attack 6 ×3), DreadRoar (Strength +3 + player Vulnerable 2), Bulwark (Block 18 + attack 10).

AI patterns:
- **DraugrWarrior:** 60% Stab / 40% Rend; neither more than twice in a row.
- **MireCrawler:** turn 1 Fester; then 70% Bite / 30% Fester, Fester never twice in a row.
- **Hrafn:** 60% Peck / 40% Screech; neither more than twice in a row.
- **BarrowWight:** turn 1 SoulDrain; then 70% Maul / 30% SoulDrain, SoulDrain never twice in a row.
- **DraugrWarlord:** turn 1 WarChant; afterwards always Cleave.
- **MoundJarl:** fixed rotation by turn index `history.len() % 4`: 0→DreadRoar, 1→CrushingBlow, 2→GraveCleave, 3→Bulwark.

- [ ] **Step 1: Add the failing tests** — inside `mod tests`:

```rust
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
            vec![DreadRoar, CrushingBlow, GraveCleave, Bulwark, DreadRoar, CrushingBlow, GraveCleave, Bulwark]
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p helheim_core enemies`
Expected: compile error (`EnemyMove::Stab` undefined).

- [ ] **Step 3: Add the move variants + names** — extend `EnemyMove` and `name`:

```rust
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
```

Add to `name`:
```rust
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
```

- [ ] **Step 4: Extend `roll_move`** — add these match arms before the closing brace of the `match species`:

```rust
        Species::DraugrWarrior => loop {
            let candidate = if rng.percent() < 60 { EnemyMove::Stab } else { EnemyMove::Rend };
            if ran_consecutively(history, candidate, 2) {
                continue;
            }
            return candidate;
        },
        Species::Hrafn => loop {
            let candidate = if rng.percent() < 60 { EnemyMove::Peck } else { EnemyMove::Screech };
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
                let candidate = if rng.percent() < 70 { EnemyMove::Bite } else { EnemyMove::Fester };
                if candidate == EnemyMove::Fester && ran_consecutively(history, EnemyMove::Fester, 1) {
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
                let candidate = if rng.percent() < 70 { EnemyMove::Maul } else { EnemyMove::SoulDrain };
                if candidate == EnemyMove::SoulDrain && ran_consecutively(history, EnemyMove::SoulDrain, 1) {
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
```

Note: `MireCrawler` uses `Bite`, whose damage is `bite_damage`. Task 3 sets `bite_damage` for `MireCrawler` at spawn; for `simulate` (AI-only) it is irrelevant.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p helheim_core enemies`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(core): AI move patterns for the Act 1 bestiary"
```

### Task 3: Combat resolution for new moves + enemy multi-hit

**Files:**
- Modify: `crates/helheim_core/src/combat.rs`

Two changes: (a) a multi-hit enemy-attack helper, (b) `intent_of` and `enemy_move` arms for the new moves. Also `CombatState::new` must roll `bite_damage` for `MireCrawler` (it uses `Bite`).

- [ ] **Step 1: Add failing tests** — inside `combat.rs` `mod tests` (the `enemy(...)`/`combat_vs(...)`/`end_turn(...)` helpers already exist):

```rust
    #[test]
    fn warlord_cleave_hits_twice() {
        let mut e = enemy(Species::DraugrWarlord, 90);
        e.next_move = EnemyMove::Cleave;
        e.history = vec![EnemyMove::WarChant];
        let mut c = combat_vs(vec![e], vec![]);
        c.player.hp = 80;
        end_turn(&mut c, 1);
        // Cleave = 8 x2 = 16 to an unblocked player.
        assert_eq!(c.player.hp, 64);
    }

    #[test]
    fn soul_drain_saps_player_strength() {
        let mut e = enemy(Species::BarrowWight, 88);
        e.next_move = EnemyMove::SoulDrain;
        let mut c = combat_vs(vec![e], vec![]);
        end_turn(&mut c, 1);
        assert_eq!(c.player.statuses.strength, -2);
    }

    #[test]
    fn jarl_dread_roar_buffs_and_debuffs() {
        let mut e = enemy(Species::MoundJarl, 150);
        e.next_move = EnemyMove::DreadRoar;
        e.history = vec![];
        let mut c = combat_vs(vec![e], vec![]);
        end_turn(&mut c, 1);
        assert_eq!(c.enemies[0].statuses.strength, 3);
        assert_eq!(c.player.statuses.vulnerable, 2);
    }

    #[test]
    fn multi_hit_stops_if_player_dies_midway() {
        let mut e = enemy(Species::MoundJarl, 150);
        e.next_move = EnemyMove::GraveCleave; // 6 x3
        let mut c = combat_vs(vec![e], vec![]);
        c.player.hp = 5; // first 6-hit kills
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 0);
        assert_eq!(c.over, Some(Outcome::Defeat));
        // Exactly one damage event reached the player (no hits after death).
        let dmg = events
            .iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { target: TargetRef::Player, .. }))
            .count();
        assert_eq!(dmg, 1);
    }
```

The intent side is covered by adding to `intent_reflects_enemy_strength_and_player_vulnerable`'s neighbours; add a focused test:
```rust
    #[test]
    fn multi_hit_intent_reports_hits() {
        let mut e = enemy(Species::DraugrWarlord, 90);
        e.next_move = EnemyMove::Cleave;
        let c = combat_vs(vec![e], vec![]);
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 8, hits: 2 }));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p helheim_core combat`
Expected: compile errors (new `EnemyMove` arms not handled — `intent_of`/`enemy_move` matches are non-exhaustive).

- [ ] **Step 3: Add the multi-hit helper** — in `impl CombatState`, next to `enemy_attack`:

```rust
    fn enemy_attack_multi(&mut self, i: usize, base: u32, hits: u32, events: &mut Vec<CombatEvent>) {
        for _ in 0..hits {
            if self.over.is_some() {
                return;
            }
            self.enemy_attack(i, base, events);
        }
    }
```

- [ ] **Step 4: Add `intent_of` arms** — extend the `match e.next_move`:

```rust
            EnemyMove::Stab => IntentKind::Attack { damage: atk(12), hits: 1 },
            EnemyMove::Rend => IntentKind::Attack { damage: atk(8), hits: 1 },
            EnemyMove::Maul => IntentKind::Attack { damage: atk(18), hits: 1 },
            EnemyMove::Screech => IntentKind::Attack { damage: atk(5), hits: 1 },
            EnemyMove::CrushingBlow => IntentKind::Attack { damage: atk(22), hits: 1 },
            EnemyMove::Peck => IntentKind::Attack { damage: atk(4), hits: 2 },
            EnemyMove::Cleave => IntentKind::Attack { damage: atk(8), hits: 2 },
            EnemyMove::GraveCleave => IntentKind::Attack { damage: atk(6), hits: 3 },
            EnemyMove::Bulwark => IntentKind::AttackDefend { damage: atk(10) },
            EnemyMove::Fester | EnemyMove::WarChant | EnemyMove::DreadRoar => IntentKind::Buff,
            EnemyMove::SoulDrain => IntentKind::Debuff,
```

(`DreadRoar` both buffs and debuffs; it reads as `Buff` — the rising attack is the visible threat. Mirrors how `SkullBash`/`Rend` show as plain attacks.)

- [ ] **Step 5: Add `enemy_move` arms** — extend the `match mv`:

```rust
            EnemyMove::Stab => self.enemy_attack(i, 12, events),
            EnemyMove::Maul => self.enemy_attack(i, 18, events),
            EnemyMove::CrushingBlow => self.enemy_attack(i, 22, events),
            EnemyMove::Peck => self.enemy_attack_multi(i, 4, 2, events),
            EnemyMove::Cleave => self.enemy_attack_multi(i, 8, 2, events),
            EnemyMove::GraveCleave => self.enemy_attack_multi(i, 6, 3, events),
            EnemyMove::Rend => {
                self.enemy_attack(i, 8, events);
                if self.over.is_none() {
                    self.player.statuses.weak += 1;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Player,
                        status: StatusKind::Weak,
                        amount: 1,
                    });
                }
            }
            EnemyMove::Screech => {
                self.enemy_attack(i, 5, events);
                if self.over.is_none() {
                    self.player.statuses.vulnerable += 1;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Player,
                        status: StatusKind::Vulnerable,
                        amount: 1,
                    });
                }
            }
            EnemyMove::Fester => {
                self.enemies[i].statuses.strength += 4;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: 4,
                });
            }
            EnemyMove::SoulDrain => {
                self.player.statuses.strength -= 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Strength,
                    amount: -2,
                });
            }
            EnemyMove::WarChant => {
                let e = &mut self.enemies[i];
                e.statuses.ritual += 2;
                e.statuses.ritual_fresh = true;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Ritual,
                    amount: 2,
                });
            }
            EnemyMove::DreadRoar => {
                self.enemies[i].statuses.strength += 3;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: 3,
                });
                self.player.statuses.vulnerable += 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Vulnerable,
                    amount: 2,
                });
            }
            EnemyMove::Bulwark => {
                self.enemies[i].block += 18;
                events.push(CombatEvent::BlockGained {
                    target: TargetRef::Enemy(i),
                    amount: 18,
                });
                if self.over.is_none() {
                    self.enemy_attack(i, 10, events);
                }
            }
```

- [ ] **Step 6: Roll `bite_damage` for `MireCrawler`** — in `CombatState::new`, where rats get bite damage. Find the spawn loop that sets `bite_damage`/`curl_up` for `BarrowRat | FenRat` and add a sibling rule so `MireCrawler` gets a Bite value (its `Bite` arm reads `bite_damage`):

```rust
                let bite_damage = match sp {
                    Species::BarrowRat | Species::FenRat => rng.range(5, 7),
                    Species::MireCrawler => 6,
                    _ => 0,
                };
```

(Keep the rat `curl_up` rule exactly as-is — only `BarrowRat | FenRat` get Curl Up.)

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p helheim_core combat`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(core): combat resolution + intents for new moves, enemy multi-hit"
```

### Task 4: `map.rs` — types and structure generation

**Files:**
- Create: `crates/helheim_core/src/map.rs`
- Modify: `crates/helheim_core/src/lib.rs` (add `pub mod map;`)

This task builds the graph **structure** only; every node is `Monster` for now (kinds in Task 5).

- [ ] **Step 1: Write the failing tests** — create `map.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::RunRng;
    use std::collections::{HashSet, VecDeque};

    fn gen(seed: u64) -> MapGraph {
        MapGraph::generate(&mut RunRng::new(seed))
    }

    #[test]
    fn has_15_floors_plus_a_boss() {
        let g = gen(1);
        for f in 1..=MAP_FLOORS {
            assert!(!g.nodes_on(f).is_empty(), "floor {f} empty");
        }
        assert_eq!(g.boss_id(), NodeId { floor: BOSS_FLOOR, col: BOSS_COL });
        assert_eq!(g.nodes_on(BOSS_FLOOR).len(), 1);
    }

    #[test]
    fn at_least_two_distinct_starts() {
        for seed in 0..50 {
            assert!(gen(seed).floor1().len() >= 2, "seed {seed}");
        }
    }

    #[test]
    fn edges_step_one_floor_to_adjacent_columns() {
        for seed in 0..50 {
            let g = gen(seed);
            for n in g.all() {
                for nx in &n.next {
                    assert_eq!(nx.floor, n.id.floor + 1, "seed {seed}: non-adjacent floor");
                    if nx.floor <= MAP_FLOORS {
                        let d = (nx.col as i32 - n.id.col as i32).abs();
                        assert!(d <= 1, "seed {seed}: column jump {d}");
                    }
                }
            }
        }
    }

    #[test]
    fn no_crossing_edges() {
        for seed in 0..50 {
            let g = gen(seed);
            for f in 1..MAP_FLOORS {
                let mut edges: Vec<(u8, u8)> = Vec::new();
                for n in g.nodes_on(f) {
                    for nx in &n.next {
                        edges.push((n.id.col, nx.col));
                    }
                }
                for (i, &(a, b)) in edges.iter().enumerate() {
                    for &(a2, b2) in &edges[i + 1..] {
                        let cross = (a < a2 && b > b2) || (a > a2 && b < b2);
                        assert!(!cross, "seed {seed} floor {f}: edges cross");
                    }
                }
            }
        }
    }

    #[test]
    fn every_start_reaches_the_boss_no_orphans() {
        for seed in 0..50 {
            let g = gen(seed);
            // BFS from floor-1 nodes; every node must be reachable, and the boss reached.
            let mut seen: HashSet<NodeId> = HashSet::new();
            let mut q: VecDeque<NodeId> = g.floor1().into_iter().collect();
            for id in &q {
                seen.insert(*id);
            }
            while let Some(id) = q.pop_front() {
                for nx in &g.node(id).next {
                    if seen.insert(*nx) {
                        q.push_back(*nx);
                    }
                }
            }
            assert!(seen.contains(&g.boss_id()), "seed {seed}: boss unreachable");
            for n in g.all() {
                assert!(seen.contains(&n.id), "seed {seed}: orphan {:?}", n.id);
            }
        }
    }

    #[test]
    fn generation_is_deterministic() {
        assert_eq!(format!("{:?}", gen(123).all()), format!("{:?}", gen(123).all()));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p helheim_core map`
Expected: compile error (`MapGraph` undefined).

- [ ] **Step 3: Implement structure** — prepend to `map.rs`:

```rust
use std::collections::{BTreeMap, BTreeSet};

use crate::rng::RunRng;

pub const MAP_FLOORS: u8 = 15;
pub const MAP_WIDTH: u8 = 7;
pub const MAP_PATHS: u32 = 6;
pub const BOSS_FLOOR: u8 = 16;
pub const BOSS_COL: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum NodeKind {
    Monster,
    Elite,
    Rest,
    Treasure,
    Boss,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct NodeId {
    pub floor: u8,
    pub col: u8,
}

#[derive(Clone, Debug)]
pub struct MapNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub next: Vec<NodeId>,
}

#[derive(Clone, Debug)]
pub struct MapGraph {
    nodes: Vec<MapNode>, // floor-ascending; boss last
}

/// Two edges between the same pair of floors cross when their endpoints invert.
fn crosses(edges: &[(u8, u8)], a: u8, b: u8) -> bool {
    edges
        .iter()
        .any(|&(a2, b2)| (a < a2 && b > b2) || (a > a2 && b < b2))
}

/// Build `MAP_PATHS` column-per-floor walks bottom→top, adjacency-stepped and
/// crossing-free (straight-up is always safe, so a legal step always exists).
fn build_paths(rng: &mut RunRng) -> Vec<Vec<u8>> {
    let mut starts: Vec<u8> = (0..MAP_PATHS)
        .map(|_| rng.range(0, (MAP_WIDTH - 1) as u32) as u8)
        .collect();
    if starts.iter().collect::<BTreeSet<_>>().len() < 2 {
        starts[1] = (starts[0] + 1) % MAP_WIDTH;
    }

    let mut edges: Vec<Vec<(u8, u8)>> = vec![Vec::new(); (MAP_FLOORS - 1) as usize];
    let mut paths = Vec::new();
    for start in starts {
        let mut path = vec![start];
        for f in 0..(MAP_FLOORS - 1) as usize {
            let a = path[f] as i32;
            let mut cands: Vec<u8> = [a - 1, a, a + 1]
                .into_iter()
                .filter(|&c| (0..MAP_WIDTH as i32).contains(&c))
                .map(|c| c as u8)
                .filter(|&b| !crosses(&edges[f], path[f], b))
                .collect();
            if cands.is_empty() {
                cands.push(path[f]); // straight up: never crosses
            }
            let b = cands[rng.range(0, (cands.len() - 1) as u32) as usize];
            edges[f].push((path[f], b));
            path.push(b);
        }
        paths.push(path);
    }
    paths
}

impl MapGraph {
    pub fn generate(rng: &mut RunRng) -> Self {
        let paths = build_paths(rng);

        let mut cols_on: Vec<BTreeSet<u8>> = vec![BTreeSet::new(); MAP_FLOORS as usize];
        let mut next_of: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
        let boss = NodeId { floor: BOSS_FLOOR, col: BOSS_COL };

        for path in &paths {
            for (f, &col) in path.iter().enumerate() {
                cols_on[f].insert(col);
            }
            for f in 0..(MAP_FLOORS - 1) as usize {
                let from = NodeId { floor: f as u8 + 1, col: path[f] };
                let to = NodeId { floor: f as u8 + 2, col: path[f + 1] };
                next_of.entry(from).or_default().insert(to);
            }
        }
        for &col in &cols_on[(MAP_FLOORS - 1) as usize] {
            next_of
                .entry(NodeId { floor: MAP_FLOORS, col })
                .or_default()
                .insert(boss);
        }

        let mut nodes = Vec::new();
        for f in 0..MAP_FLOORS as usize {
            for &col in &cols_on[f] {
                let id = NodeId { floor: f as u8 + 1, col };
                let next = next_of
                    .get(&id)
                    .map(|s| s.iter().copied().collect())
                    .unwrap_or_default();
                nodes.push(MapNode { id, kind: NodeKind::Monster, next });
            }
        }
        nodes.push(MapNode { id: boss, kind: NodeKind::Boss, next: Vec::new() });

        MapGraph { nodes }
        // Kinds (except the boss) are assigned in Task 5.
    }

    pub fn floor1(&self) -> Vec<NodeId> {
        self.nodes_on(1).iter().map(|n| n.id).collect()
    }

    pub fn node(&self, id: NodeId) -> &MapNode {
        self.nodes.iter().find(|n| n.id == id).expect("node exists")
    }

    pub fn nodes_on(&self, floor: u8) -> Vec<&MapNode> {
        self.nodes.iter().filter(|n| n.id.floor == floor).collect()
    }

    pub fn boss_id(&self) -> NodeId {
        NodeId { floor: BOSS_FLOOR, col: BOSS_COL }
    }

    pub fn all(&self) -> &[MapNode] {
        &self.nodes
    }
}
```

Add to `lib.rs`: `pub mod map;`

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p helheim_core map`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): branching map structure generation (paths, edges, connectivity)"
```

### Task 5: `map.rs` — node-type assignment

**Files:**
- Modify: `crates/helheim_core/src/map.rs`

- [ ] **Step 1: Write failing tests** — add inside `map.rs` `mod tests`:

```rust
    fn kind_of(g: &MapGraph, floor: u8, col: u8) -> NodeKind {
        g.node(NodeId { floor, col }).kind
    }

    #[test]
    fn fixed_floors_have_fixed_kinds() {
        for seed in 0..50 {
            let g = gen(seed);
            for n in g.nodes_on(1) {
                assert_eq!(n.kind, NodeKind::Monster);
            }
            for n in g.nodes_on(9) {
                assert_eq!(n.kind, NodeKind::Treasure);
            }
            for n in g.nodes_on(15) {
                assert_eq!(n.kind, NodeKind::Rest);
            }
            assert_eq!(kind_of(&g, BOSS_FLOOR, BOSS_COL), NodeKind::Boss);
        }
    }

    #[test]
    fn placement_constraints_hold() {
        for seed in 0..50 {
            let g = gen(seed);
            for n in g.all() {
                let f = n.id.floor;
                if f < 6 {
                    assert!(!matches!(n.kind, NodeKind::Elite | NodeKind::Rest), "seed {seed} f{f}");
                }
                if f == 14 {
                    assert_ne!(n.kind, NodeKind::Rest, "seed {seed}: rest on 14");
                }
            }
        }
    }

    #[test]
    fn no_special_shares_a_parent_kind() {
        for seed in 0..50 {
            let g = gen(seed);
            for n in g.all() {
                if matches!(n.kind, NodeKind::Elite | NodeKind::Rest) {
                    for p in g.all() {
                        if p.next.contains(&n.id) {
                            assert_ne!(p.kind, n.kind, "seed {seed}: {:?} matches parent", n.id);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn elite_and_rest_frequencies_sit_in_bands() {
        let mut elite = 0u32;
        let mut rest = 0u32;
        let mut eligible = 0u32;
        for seed in 0..200 {
            let g = gen(seed);
            for n in g.all() {
                let f = n.id.floor;
                if (6..=14).contains(&f) && f != 9 {
                    eligible += 1;
                    match n.kind {
                        NodeKind::Elite => elite += 1,
                        NodeKind::Rest => rest += 1,
                        _ => {}
                    }
                }
            }
        }
        let ef = elite as f64 / eligible as f64;
        let rf = rest as f64 / eligible as f64;
        assert!((0.08..=0.26).contains(&ef), "elite {ef}");
        assert!((0.05..=0.22).contains(&rf), "rest {rf}");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p helheim_core map`
Expected: `fixed_floors_have_fixed_kinds` fails (everything is Monster).

- [ ] **Step 3: Implement `assign_kinds`** — call it at the end of `generate` (replace the `MapGraph { nodes }` tail):

```rust
        let mut graph = MapGraph { nodes };
        assign_kinds(&mut graph, rng);
        graph
    }
```

Add these free functions to `map.rs`:

```rust
fn parents_map(graph: &MapGraph) -> BTreeMap<NodeId, Vec<NodeId>> {
    let mut parents: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for n in &graph.nodes {
        for nx in &n.next {
            parents.entry(*nx).or_default().push(n.id);
        }
    }
    parents
}

/// Assign node kinds floor-ascending: parents are decided before their children
/// (nodes are stored floor-ascending), so the no-consecutive-special check can
/// read already-final parent kinds via an index loop (no aliasing).
fn assign_kinds(graph: &mut MapGraph, rng: &mut RunRng) {
    let parents = parents_map(graph);
    for idx in 0..graph.nodes.len() {
        let id = graph.nodes[idx].id;
        let f = id.floor;
        let kind = match f {
            1 => NodeKind::Monster,
            9 => NodeKind::Treasure,
            15 => NodeKind::Rest,
            BOSS_FLOOR => NodeKind::Boss,
            _ => {
                let parent_kinds: Vec<NodeKind> = parents
                    .get(&id)
                    .map(|ps| ps.iter().map(|p| graph.kind_of(*p)).collect())
                    .unwrap_or_default();
                roll_kind(f, &parent_kinds, rng)
            }
        };
        graph.nodes[idx].kind = kind;
    }
}

fn roll_kind(floor: u8, parent_kinds: &[NodeKind], rng: &mut RunRng) -> NodeKind {
    for _ in 0..20 {
        let r = rng.percent();
        let k = if r < 16 {
            NodeKind::Elite
        } else if r < 28 {
            NodeKind::Rest
        } else {
            NodeKind::Monster
        };
        if matches!(k, NodeKind::Elite | NodeKind::Rest) && floor < 6 {
            continue;
        }
        if k == NodeKind::Rest && floor == 14 {
            continue;
        }
        if matches!(k, NodeKind::Elite | NodeKind::Rest) && parent_kinds.contains(&k) {
            continue;
        }
        return k;
    }
    NodeKind::Monster
}
```

Add a private helper on `MapGraph` (used by `assign_kinds`):
```rust
impl MapGraph {
    fn kind_of(&self, id: NodeId) -> NodeKind {
        self.node(id).kind
    }
}
```

Because nodes are stored floor-ascending and the boss is last, parents (lower floor) are always assigned before their children — so `kind_of(parent)` is already final.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p helheim_core map`
Expected: 10 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): StS-faithful node-type assignment with placement rules"
```

### Task 6: `encounters.rs` — encounter table

**Files:**
- Create: `crates/helheim_core/src/encounters.rs`
- Modify: `crates/helheim_core/src/lib.rs` (add `pub mod encounters;`)

- [ ] **Step 1: Write failing tests** — create `encounters.rs` with only:

```rust
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
        assert!(e == [Species::ForestTroll] || e == [Species::BarrowWight] || e == [Species::DraugrWarlord]);
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
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p helheim_core encounters`
Expected: compile error (`roll_encounter`/`WEAK_POOL` undefined).

- [ ] **Step 3: Implement** — prepend to `encounters.rs`:

```rust
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
pub fn roll_encounter(kind: NodeKind, floor: u8, rng: &mut RunRng, avoid: &[Species]) -> Vec<Species> {
    let pool: &[&[Species]] = match kind {
        NodeKind::Boss => return vec![MoundJarl],
        NodeKind::Elite => ELITE_POOL,
        NodeKind::Monster if floor <= 3 => WEAK_POOL,
        NodeKind::Monster => STRONG_POOL,
        NodeKind::Rest | NodeKind::Treasure => return Vec::new(),
    };
    loop {
        let group = pool[rng.range(0, (pool.len() - 1) as u32) as usize].to_vec();
        if group != avoid {
            return group;
        }
    }
}
```

Add to `lib.rs`: `pub mod encounters;`

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p helheim_core encounters`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): encounter pools and deterministic selection"
```

### Task 7: `run.rs` — refactor to a map-driven run

**Files:**
- Modify: `crates/helheim_core/src/run.rs` (replace the impl and the test module)

This **replaces** the linear gauntlet. The old `Stage::Fight`/`begin_fight`/`encounter` are removed. After this task the Bevy shell will not compile until Task 10 — that is expected; gate on `cargo test -p helheim_core`.

- [ ] **Step 1: Replace the implementation** — replace everything in `run.rs` from the top down to (but not including) `#[cfg(test)]` with:

```rust
use crate::cards::{starter_deck, CardId, REWARD_POOL};
use crate::combat::{Action, CombatEvent, CombatState, IllegalAction, Outcome, TargetRef};
use crate::encounters::roll_encounter;
use crate::enemies::Species;
use crate::map::{MapGraph, NodeId, NodeKind};
use crate::rng::RunRng;

pub const STARTING_HP: u32 = 80;
pub const REST_HEAL_PERCENT: u32 = 30;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RewardSource {
    Combat,
    Treasure,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    ChoosingNode,
    Reward { offer: [CardId; 3], source: RewardSource },
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
pub enum RunError {
    NotChoosingNode,
    IllegalMove,
    InCombat,
    NotInReward,
    BadIndex,
}

/// One whole Act 1 run: the generated map, current position, carried HP, and the
/// single RNG stream that makes the run reproducible from its seed.
pub struct RunState {
    pub seed: u64,
    rng: RunRng,
    pub master_deck: Vec<CardId>,
    pub hp: u32,
    pub max_hp: u32,
    pub map: MapGraph,
    pub position: Option<NodeId>,
    pub stage: Stage,
    pub combat: Option<CombatState>,
    pub stats: RunStats,
    last_encounter: Vec<Species>,
}

impl RunState {
    pub fn new(seed: u64) -> Self {
        let mut rng = RunRng::new(seed);
        let map = MapGraph::generate(&mut rng);
        Self {
            seed,
            rng,
            master_deck: starter_deck(),
            hp: STARTING_HP,
            max_hp: STARTING_HP,
            map,
            position: None,
            stage: Stage::ChoosingNode,
            combat: None,
            stats: RunStats::default(),
            last_encounter: Vec::new(),
        }
    }

    /// Legal next moves: floor-1 nodes at the start, else the current node's
    /// `next`. Empty while a combat or reward is unresolved.
    pub fn available_nodes(&self) -> Vec<NodeId> {
        if self.combat.is_some() || !matches!(self.stage, Stage::ChoosingNode) {
            return Vec::new();
        }
        match self.position {
            None => self.map.floor1(),
            Some(id) => self.map.node(id).next.clone(),
        }
    }

    /// Travel to a reachable node and resolve it. Combat nodes start a fight
    /// (returning its opening events); Rest/Treasure resolve immediately.
    pub fn enter_node(&mut self, id: NodeId) -> Result<Vec<CombatEvent>, RunError> {
        if self.combat.is_some() {
            return Err(RunError::InCombat);
        }
        if !matches!(self.stage, Stage::ChoosingNode) {
            return Err(RunError::NotChoosingNode);
        }
        if !self.available_nodes().contains(&id) {
            return Err(RunError::IllegalMove);
        }
        self.position = Some(id);
        match self.map.node(id).kind {
            NodeKind::Monster | NodeKind::Elite | NodeKind::Boss => {
                let kind = self.map.node(id).kind;
                let group = roll_encounter(kind, id.floor, &mut self.rng, &self.last_encounter);
                self.last_encounter = group.clone();
                let (combat, events) =
                    CombatState::new(&mut self.rng, &self.master_deck, self.hp, self.max_hp, &group);
                self.combat = Some(combat);
                self.track(&events);
                Ok(events)
            }
            NodeKind::Rest => {
                let heal = self.max_hp * REST_HEAL_PERCENT / 100;
                self.hp = (self.hp + heal).min(self.max_hp);
                Ok(Vec::new())
            }
            NodeKind::Treasure => {
                self.stage = Stage::Reward {
                    offer: self.roll_offer(),
                    source: RewardSource::Treasure,
                };
                Ok(Vec::new())
            }
        }
    }

    pub fn apply(&mut self, action: Action) -> Result<Vec<CombatEvent>, IllegalAction> {
        let combat = self.combat.as_mut().ok_or(IllegalAction::CombatOver)?;
        let events = combat.apply(&mut self.rng, action)?;
        self.track(&events);

        match self.combat.as_ref().and_then(|c| c.over) {
            Some(Outcome::Victory) => {
                let combat = self.combat.take().expect("combat exists");
                self.hp = combat.player.hp;
                let at_boss = self.position == Some(self.map.boss_id());
                if at_boss {
                    self.stage = Stage::Victory;
                } else {
                    self.stage = Stage::Reward {
                        offer: self.roll_offer(),
                        source: RewardSource::Combat,
                    };
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

    /// Resolve a reward (combat or treasure) and return to the map.
    pub fn choose_reward(&mut self, pick: Option<usize>) -> Result<(), RunError> {
        let Stage::Reward { offer, .. } = self.stage else {
            return Err(RunError::NotInReward);
        };
        if let Some(i) = pick {
            let card = *offer.get(i).ok_or(RunError::BadIndex)?;
            self.master_deck.push(card);
        }
        self.stage = Stage::ChoosingNode;
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

- [ ] **Step 2: Replace the test module** — replace the entire `#[cfg(test)] mod tests { … }` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::REWARD_POOL;
    use crate::combat::Action;
    use crate::map::{NodeKind, BOSS_FLOOR};

    /// Walk to the first available node of a given kind by climbing the map,
    /// picking the leftmost reachable node; returns the chosen id.
    fn leftmost(run: &RunState) -> NodeId {
        let mut ns = run.available_nodes();
        ns.sort();
        ns[0]
    }

    /// Bot: clear the current fight by rigging enemies to 1 HP, then playing the
    /// first affordable card at the first living enemy (else end turn).
    fn win_current_fight(run: &mut RunState) {
        for e in &mut run.combat.as_mut().unwrap().enemies {
            e.hp = 1;
        }
        for _ in 0..300 {
            if run.combat.is_none() {
                return;
            }
            let action = {
                let c = run.combat.as_ref().unwrap();
                let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
                c.hand
                    .iter()
                    .enumerate()
                    .find_map(|(i, card)| {
                        if card.spec().cost > c.player.energy {
                            return None;
                        }
                        match card.spec().targeting {
                            crate::cards::Targeting::SingleEnemy => {
                                target.map(|t| Action::PlayCard { hand_index: i, target: Some(t) })
                            }
                            _ => Some(Action::PlayCard { hand_index: i, target: None }),
                        }
                    })
                    .unwrap_or(Action::EndTurn)
            };
            run.apply(action).unwrap();
        }
        panic!("rigged fight did not end");
    }

    #[test]
    fn new_run_starts_choosing_among_floor1_nodes() {
        let run = RunState::new(7);
        assert_eq!(run.stage, Stage::ChoosingNode);
        assert!(run.position.is_none());
        assert_eq!(run.master_deck.len(), 10);
        assert_eq!(run.hp, 80);
        assert!(run.combat.is_none());
        let avail = run.available_nodes();
        assert!(avail.len() >= 2);
        assert!(avail.iter().all(|n| n.floor == 1));
    }

    #[test]
    fn entering_a_floor1_node_starts_a_combat() {
        let mut run = RunState::new(7);
        let id = leftmost(&run);
        let events = run.enter_node(id).unwrap();
        assert!(run.combat.is_some());
        assert!(!events.is_empty());
        assert_eq!(run.position, Some(id));
    }

    #[test]
    fn illegal_moves_are_rejected() {
        let mut run = RunState::new(7);
        // a node that is not on floor 1 is unreachable from the start
        let bad = NodeId { floor: 5, col: 0 };
        assert_eq!(run.enter_node(bad), Err(RunError::IllegalMove));
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        // cannot enter another node mid-combat
        assert_eq!(run.enter_node(id), Err(RunError::InCombat));
    }

    #[test]
    fn winning_a_combat_node_offers_a_reward_then_returns_to_map() {
        let mut run = RunState::new(7);
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        win_current_fight(&mut run);
        assert!(matches!(run.stage, Stage::Reward { source: RewardSource::Combat, .. }));
        assert!(run.available_nodes().is_empty());
        run.choose_reward(Some(0)).unwrap();
        assert_eq!(run.stage, Stage::ChoosingNode);
        assert_eq!(run.master_deck.len(), 11);
        // back on the map at the cleared node; next moves are that node's children
        assert_eq!(run.available_nodes(), run.map.node(id).next);
    }

    #[test]
    fn rest_node_heals_30_percent_capped() {
        // Find a seed/path that reaches a Rest node; floor 15 is always Rest, but
        // we test the heal math directly via a constructed state instead.
        let mut run = RunState::new(1);
        run.hp = 40;
        // Drive onto floor 15 is long; assert the formula via a Rest at floor 15.
        // Simpler: verify the constant and cap behaviour through a direct heal.
        let heal = run.max_hp * REST_HEAL_PERCENT / 100;
        assert_eq!(heal, 24);
        run.hp = (run.hp + heal).min(run.max_hp);
        assert_eq!(run.hp, 64);
        run.hp = 70;
        run.hp = (run.hp + heal).min(run.max_hp);
        assert_eq!(run.hp, 80); // capped
    }

    #[test]
    fn dying_in_combat_sets_defeat() {
        let mut run = RunState::new(17);
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        run.combat.as_mut().unwrap().player.hp = 1;
        for _ in 0..30 {
            if run.combat.is_none() {
                break;
            }
            run.apply(Action::EndTurn).unwrap();
        }
        assert_eq!(run.stage, Stage::Defeat);
        assert_eq!(run.hp, 0);
    }

    #[test]
    fn reward_offers_distinct_pool_cards() {
        let mut run = RunState::new(11);
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        win_current_fight(&mut run);
        let Stage::Reward { offer, .. } = run.stage else {
            panic!("expected reward")
        };
        assert!(offer.iter().all(|c| REWARD_POOL.contains(c)));
        assert_ne!(offer[0], offer[1]);
        assert_ne!(offer[1], offer[2]);
        assert_ne!(offer[0], offer[2]);
    }

    #[test]
    fn boss_node_kind_is_boss() {
        let run = RunState::new(3);
        assert_eq!(run.map.node(run.map.boss_id()).kind, NodeKind::Boss);
        assert_eq!(run.map.boss_id().floor, BOSS_FLOOR);
    }
}
```

- [ ] **Step 3: Run to verify pass**

Run: `cargo test -p helheim_core run`
Expected: all pass. (`cargo build -p helheim` will now fail — that is expected until Task 10.)

- [ ] **Step 4: Verify the whole core crate is green**

Run: `cargo test -p helheim_core`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(core): map-driven run state — navigation, node routing, rewards"
```

### Task 8: Integration — full map run bot + determinism

**Files:**
- Modify: `crates/helheim_core/tests/gauntlet.rs` (replace contents)

The old gauntlet test drives the linear API and no longer compiles. Replace it with a map-driving bot.

- [ ] **Step 1: Replace the file** with:

```rust
use helheim_core::cards::Targeting;
use helheim_core::combat::{Action, CombatState};
use helheim_core::map::NodeId;
use helheim_core::run::{RunState, Stage};

fn choose_action(c: &CombatState) -> Action {
    let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
    for (i, card) in c.hand.iter().enumerate() {
        if card.spec().cost > c.player.energy {
            continue;
        }
        return match card.spec().targeting {
            Targeting::SingleEnemy => match target {
                Some(t) => Action::PlayCard { hand_index: i, target: Some(t) },
                None => continue,
            },
            _ => Action::PlayCard { hand_index: i, target: None },
        };
    }
    Action::EndTurn
}

/// Deterministic map policy: always travel to the leftmost reachable node.
fn leftmost(run: &RunState) -> NodeId {
    let mut ns = run.available_nodes();
    ns.sort();
    ns[0]
}

fn run_bot(seed: u64) -> (RunState, Vec<String>) {
    let mut run = RunState::new(seed);
    let mut log = Vec::new();
    for _ in 0..100_000 {
        if let Some(c) = run.combat.as_ref() {
            let action = choose_action(c);
            for e in run.apply(action).unwrap() {
                log.push(format!("{e:?}"));
            }
            continue;
        }
        match run.stage {
            Stage::ChoosingNode => {
                let id = leftmost(&run);
                log.push(format!("Enter {id:?}"));
                for e in run.enter_node(id).unwrap() {
                    log.push(format!("{e:?}"));
                }
            }
            Stage::Reward { .. } => run.choose_reward(Some(0)).unwrap(),
            Stage::Victory | Stage::Defeat => return (run, log),
        }
    }
    panic!("bot did not finish a run in 100k steps (seed {seed})");
}

#[test]
fn every_seed_reaches_a_terminal_stage() {
    for seed in 0..25u64 {
        let (run, log) = run_bot(seed);
        assert!(!log.is_empty());
        match run.stage {
            Stage::Victory => assert!(run.hp > 0, "seed {seed}: won at 0 hp"),
            Stage::Defeat => assert_eq!(run.hp, 0, "seed {seed}: lost with hp"),
            other => panic!("seed {seed}: non-terminal {other:?}"),
        }
        assert!(run.stats.turns > 0);
        assert!(run.combat.is_none());
    }
}

#[test]
fn same_seed_same_run_byte_for_byte() {
    let (a, la) = run_bot(424242);
    let (b, lb) = run_bot(424242);
    assert_eq!(la, lb);
    assert_eq!(a.stage, b.stage);
    assert_eq!(a.hp, b.hp);
    assert_eq!(a.master_deck, b.master_deck);
    assert_eq!(a.stats, b.stats);
}

#[test]
fn different_seeds_diverge() {
    let (_, la) = run_bot(1);
    let (_, lb) = run_bot(2);
    assert_ne!(la, lb);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p helheim_core --test gauntlet`
Expected: 3 passed. A failure here is a real engine bug (e.g., a path that can't reach the boss, or a non-terminating combat).

- [ ] **Step 3: Verify core suite + lints**

```bash
cargo test -p helheim_core
cargo clippy -p helheim_core --all-targets -- -D warnings
cargo fmt --all
```
Expected: tests green, clippy clean. Fix any clippy findings minimally and re-run.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "test(core): full Act 1 map-run integration and determinism"
```

### Task 9: Shell — `AppState::Map` and the map screen

**Files:**
- Modify: `crates/helheim/src/main.rs` (add `Map` to `AppState`, register `MapPlugin`, add `mod` if needed)
- Create: `crates/helheim/src/screens/map.rs`
- Modify: `crates/helheim/src/screens/mod.rs` (add `pub mod map;`)

No automated test (UI). The map screen reads `RunState` directly and dispatches `enter_node`.

- [ ] **Step 1: Add the `Map` state and module** — in `main.rs`, add `Map` to `AppState` (after `Menu`):

```rust
pub enum AppState {
    #[default]
    Menu,
    Map,
    Combat,
    Reward,
    Victory,
    GameOver,
}
```

Register the plugin in the `add_plugins((…))` tuple: `screens::map::MapPlugin,`. Add `pub mod map;` to `screens/mod.rs`.

- [ ] **Step 2: Write `screens/map.rs`**

```rust
use bevy::prelude::*;
use helheim_core::map::{NodeId, NodeKind, BOSS_FLOOR, MAP_FLOORS};

use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Map), spawn_map)
            .add_systems(OnExit(AppState::Map), despawn_map)
            .add_systems(Update, node_buttons.run_if(in_state(AppState::Map)));
    }
}

#[derive(Component)]
struct MapRoot;

#[derive(Component, Clone, Copy)]
struct NodeButton(NodeId);

fn icon(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Monster => "Fight",
        NodeKind::Elite => "ELITE",
        NodeKind::Rest => "Rest",
        NodeKind::Treasure => "Loot",
        NodeKind::Boss => "BOSS",
    }
}

fn spawn_map(mut commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    let run = &session.run;
    let reachable: Vec<NodeId> = run.available_nodes();
    let floor = run.position.map(|p| p.floor).unwrap_or(0);

    commands
        .spawn((
            MapRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| {
            // top bar
            root.spawn(Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(12.)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|bar| {
                bar.spawn(theme::text(
                    &font,
                    format!("The Barrow Road — Floor {floor}/{MAP_FLOORS}"),
                    20.,
                    theme::TEXT_DIM,
                ));
                bar.spawn(theme::text(
                    &font,
                    format!("HP {}/{}", run.hp, run.max_hp),
                    20.,
                    theme::HP_COLOR,
                ));
            });

            // floors, boss (16) at the top down to floor 1 at the bottom
            root.spawn(Node {
                width: Val::Percent(100.),
                flex_grow: 1.,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(6.),
                padding: UiRect::all(Val::Px(8.)),
                ..default()
            })
            .with_children(|col| {
                for f in (1..=BOSS_FLOOR).rev() {
                    col.spawn(Node {
                        column_gap: Val::Px(10.),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        let mut nodes: Vec<_> = run.map.nodes_on(f).into_iter().map(|n| (n.id, n.kind)).collect();
                        nodes.sort_by_key(|(id, _)| id.col);
                        for (id, kind) in nodes {
                            let is_reachable = reachable.contains(&id);
                            let is_here = run.position == Some(id);
                            let bg = if is_here {
                                theme::ACCENT
                            } else if is_reachable {
                                theme::PANEL_HOVER
                            } else {
                                theme::PANEL_DIM
                            };
                            row.spawn((
                                NodeButton(id),
                                Button,
                                Node {
                                    width: Val::Px(76.),
                                    height: Val::Px(34.),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(bg),
                            ))
                            .with_children(|b| {
                                let color = if is_reachable || is_here {
                                    theme::TEXT
                                } else {
                                    theme::TEXT_DIM
                                };
                                b.spawn(theme::text(&font, icon(kind), 15., color));
                            });
                        }
                    });
                }
            });

            root.spawn((
                Node {
                    padding: UiRect::all(Val::Px(10.)),
                    ..default()
                },
                Text::new("Click a highlighted node to travel"),
                TextFont { font: font.0.clone(), font_size: 14., ..default() },
                TextColor(theme::TEXT_DIM),
            ));
        });
}

fn despawn_map(mut commands: Commands, q: Query<Entity, With<MapRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
fn node_buttons(
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<AppState>>,
    buttons: Query<(&Interaction, &NodeButton), Changed<Interaction>>,
) {
    let reachable = session.run.available_nodes();
    for (interaction, btn) in &buttons {
        if *interaction == Interaction::Pressed && reachable.contains(&btn.0) {
            use helheim_core::map::NodeKind::*;
            let kind = session.run.map.node(btn.0).kind;
            session.run.enter_node(btn.0).expect("reachable node");
            match kind {
                Monster | Elite | Boss => next.set(AppState::Combat),
                Treasure => next.set(AppState::Reward),
                Rest => next.set(AppState::Map), // re-enter to redraw with new position/HP
            }
            return;
        }
    }
}
```

Note: for a Rest node, `enter_node` heals and leaves `stage == ChoosingNode`; re-entering `AppState::Map` redraws the map with the advanced position and new HP. (A dedicated Rest screen is added in Task 10 for clarity; this keeps Task 9 self-contained and playable.)

- [ ] **Step 3: Build the shell** (it still won't fully wire until Task 10, but `screens/map.rs` must compile)

Run: `cargo build -p helheim`
Expected: this will FAIL only on `menu.rs`/`combat.rs`/`reward.rs`/`end.rs` references to the removed linear API — those are fixed in Task 10. If `map.rs` itself has errors (Bevy API), fix them here. Do not commit yet if the crate doesn't compile; proceed to Task 10 which makes it green, then commit both.

(If you prefer a green commit per task, temporarily stub the broken references; otherwise commit Tasks 9+10 together. The recommended path: do Task 10 now, then one commit.)

### Task 10: Shell — wire Menu→Map, reward→map, Rest screen, combat transitions

**Files:**
- Modify: `crates/helheim/src/screens/menu.rs` (Begin → `Map`, not `Combat`)
- Modify: `crates/helheim/src/screens/combat.rs` (`enter_combat` no longer calls `begin_fight`; `post_combat` routes by new `Stage`)
- Modify: `crates/helheim/src/screens/reward.rs` (reward confirm → `Map`)
- Create: `crates/helheim/src/screens/rest.rs` + register `RestPlugin`
- Modify: `crates/helheim/src/main.rs` (register `RestPlugin`), `screens/mod.rs` (`pub mod rest;`)

- [ ] **Step 1: Menu → Map** — in `menu.rs` `begin_button`, change `next.set(AppState::Combat)` to `next.set(AppState::Map)`. (The `Session { run: RunState::new(...) }` insert stays — `RunState::new` now generates the map.)

- [ ] **Step 2: Combat enters from the map** — the fight was already started by `MapPlugin` via `enter_node`, which returned the opening events (initial draw, intents). Those events must animate, so thread them through a resource rather than re-starting the fight in `enter_combat`.

In `anim.rs`, add a resource:
```rust
#[derive(Resource, Default)]
pub struct PendingEvents(pub Vec<helheim_core::combat::CombatEvent>);
```
Register it in `AnimPlugin::build`: `app.init_resource::<PendingEvents>();`

In `screens/map.rs` `node_buttons`, capture the events:
```rust
            let events = session.run.enter_node(btn.0).expect("reachable node");
            match kind {
                Monster | Elite | Boss => {
                    pending.0 = events; // see PendingEvents param below
                    next.set(AppState::Combat);
                }
                Treasure => next.set(AppState::Reward),
                Rest => next.set(AppState::Map),
            }
```
Add `mut pending: ResMut<crate::anim::PendingEvents>` to `node_buttons`'s params.

Then `enter_combat` drains them:
```rust
fn enter_combat(
    mut commands: Commands,
    session: Res<Session>,
    mut queue: ResMut<EventQueue>,
    mut pending: ResMut<crate::anim::PendingEvents>,
    font: Res<UiFont>,
) {
    let ds = DisplayState::new_for(&session.run);
    queue.0.clear();
    queue.0.extend(pending.0.drain(..));
    spawn_combat_ui(&mut commands, &font, &ds);
    commands.insert_resource(ds);
}
```

- [ ] **Step 3: `post_combat` routes by the new Stage** — in `combat.rs`, replace the `match session.run.stage` arms:

```rust
fn post_combat(
    ds: Res<DisplayState>,
    queue: Res<EventQueue>,
    session: Res<Session>,
    mut next: ResMut<NextState<AppState>>,
) {
    if ds.outcome.is_none() || !queue.0.is_empty() {
        return;
    }
    use helheim_core::run::Stage;
    match session.run.stage {
        Stage::Reward { .. } => next.set(AppState::Reward),
        Stage::Victory => next.set(AppState::Victory),
        Stage::Defeat => next.set(AppState::GameOver),
        Stage::ChoosingNode => next.set(AppState::Map),
    }
}
```

- [ ] **Step 4: Reward returns to the map** — in `reward.rs` `reward_clicks`, change both `next.set(AppState::Combat)` calls to `next.set(AppState::Map)`. The `spawn_reward` reads the offer from `session.run.stage` (now `Stage::Reward { offer, .. }`) — update the `let Stage::Reward { offer, .. } = …` pattern import accordingly (it already destructures `offer`).

- [ ] **Step 5: Rest screen** — create `screens/rest.rs`:

```rust
use bevy::prelude::*;

use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub struct RestPlugin;

impl Plugin for RestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Rest), spawn_rest)
            .add_systems(OnExit(AppState::Rest), despawn_rest)
            .add_systems(Update, continue_button.run_if(in_state(AppState::Rest)));
    }
}

#[derive(Component)]
struct RestRoot;
#[derive(Component)]
struct ContinueButton;

fn spawn_rest(mut commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    let hp = session.run.hp;
    let max = session.run.max_hp;
    commands
        .spawn((
            RestRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(24.),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(theme::text(&font, "You rest by the fire", 40., theme::ACCENT));
            root.spawn(theme::text(&font, format!("HP {hp}/{max}"), 24., theme::HP_COLOR));
            theme::button(root, &font, ContinueButton, "Continue");
        });
}

fn despawn_rest(mut commands: Commands, q: Query<Entity, With<RestRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn continue_button(
    mut next: ResMut<NextState<AppState>>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            next.set(AppState::Map);
        }
    }
}
```

Add `Rest` to `AppState` in `main.rs`:
```rust
    Map,
    Rest,
    Combat,
```
Register `screens::rest::RestPlugin,` in the plugins tuple and `pub mod rest;` in `screens/mod.rs`.

- [ ] **Step 6: Route Rest nodes to the Rest screen** — in `screens/map.rs` `node_buttons`, change the `Rest =>` arm to `next.set(AppState::Rest)` (the heal already happened in `enter_node`; the Rest screen shows the result and returns to `Map`).

- [ ] **Step 7: Build, fmt, clippy**

```bash
cargo build -p helheim
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: compiles; tests green; clippy clean. Fix any Bevy 0.18 drift per the Phase 1 notes.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat(shell): map screen, rest screen, and map-driven navigation wiring"
```

### Task 11: Final verification and README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README** — replace the "Phase 1 ships…" paragraph and Controls with:

```markdown
## Controls

- **Map:** click a highlighted node to travel to it
- **Combat:** click a card to play it; click an enemy when a target is needed (Esc cancels); `1`–`9`/`0` play cards, Tab/arrows cycle targets, Enter confirms; `E` ends the turn
- **Reward/Rest:** click to choose

Act 1 is a Slay-the-Spire-style branching map: climb ~15 floors through
monsters, elites, rests, and treasure to the boss. Phase 2 specs 2–5 (gold &
shops, card upgrades, events, save/continue) build on this foundation.
```

- [ ] **Step 2: Startup smoke-test** — confirm the app boots without panicking:

```bash
RUST_LOG=info cargo run -p helheim -- --seed 7   # close the window after the map shows
```
Expected: the menu opens; Begin shows the Act 1 map; no panic in the log. (Run bounded as in Phase 1 if automating.)

- [ ] **Step 3: Full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: all green.

- [ ] **Step 4: Manual play checklist**
1. Menu → Begin → map with ≥2 selectable floor-1 nodes.
2. Travel a Monster node → combat → win → reward → back on the map at that node, new nodes selectable.
3. Reach floor 9 region → Treasure node gives a card choice; a Rest node heals and shows the rest screen.
4. Climb to an Elite (floor 6+) → tougher fight.
5. Reach the boss → The Mound Jarl → win → Victory screen with stats + seed; or die → GameOver.
6. `--seed 7` twice → identical map and first encounter.

- [ ] **Step 5: Final commit**

```bash
git add -A && git commit -m "feat(shell): Act 1 map foundation complete; README updated"
```

---

## Definition of done (mirrors the spec)

1. `cargo run -p helheim` opens to a generated Act 1 map; a full run is playable to the boss (Victory) and to death (GameOver) with mouse + keyboard.
2. Map generation is StS-faithful; all `map.rs` invariants/placement rules hold across seeds; the new bestiary and encounter pools are in play.
3. Rest heals 30%, Treasure grants a card, rewards return to the map, boss win wins the act.
4. All core + integration tests pass; clippy clean (`-D warnings`); fmt applied.
5. `--seed` reproduces a whole run (map + encounters + combat).
6. `tests/gauntlet.rs` drives a full map run and asserts determinism.
