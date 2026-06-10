# Helheim — Design Spec (Phase 1: Combat Core)

**Date:** 2026-06-10
**Status:** Approved pending user review
**Working title:** Helheim (crate `helheim`) — rename freely later

## Overview

A roguelike deck-building card game in the mold of Slay the Spire, written in Rust
with the Bevy game engine. A berserker descends through Norse realms toward Hel's
hall, fighting card battles against draugr, rats, wolves, and trolls.

**Primary goal:** learning Rust + Bevy. Clean idiomatic code, well-bounded modules,
and a thorough test suite matter as much as the game itself.

**Content strategy:** mechanics and numbers are cloned from Slay the Spire
(proven balance, verifiable against the StS wiki); names and theme are original
Norse skin. Status effects keep their standard StS names (Vulnerable, Weak,
Strength, Ritual, Enrage) so cross-referencing the wiki stays trivial; only cards,
creatures, and places are reskinned.

## Roadmap

The full StS feature set is decomposed into phases. **Each phase gets its own
spec → plan → implementation cycle.** This document fully specifies Phase 1 and
only sketches the rest.

| Phase | Delivers |
|---|---|
| **1. Combat core (this spec)** | Playable card combat in a Bevy window: deck/hand/energy, block, statuses, enemy intents and AI, multi-enemy fights, a 3-fight gauntlet with card rewards between fights, win/lose screens |
| 2. Act 1 run loop | Branching node map (~15 floors), monster/elite/rest/shop nodes, gold, card removal/upgrades, Act 1 boss, save & continue |
| 3. Relics + potions | The item layer hooking into combat events |
| 4. Events + Act 2 | Story event nodes, second realm with new enemies and boss |
| 5. Act 3 + meta | Final realm, run history, ascension-style modifiers |
| 6. Polish | Audio, animation juice, balance pass, settings |

Out of scope for Phase 1: map, shops, gold, relics, potions, events, card
upgrades, exhaust-mechanic cards, save/load, audio, sprite art, settings menu,
additional classes.

## Phase 1 — Game Design

### The player

**The Berserker** — 80 max HP, 3 energy per turn, draws 5 cards per turn,
hand limit 10 (draws beyond a full hand are forfeited). No innate healing
(the Ironclad's Burning Blood relic belongs to Phase 3).

### Combat rules (StS-faithful)

- **Turn structure:** At the start of the player's turn: player block resets
  to 0, energy refills to 3, draw 5. The player plays any number of affordable
  cards, then ends the turn: the whole hand is discarded, the player's
  end-of-turn status effects trigger (e.g. Surge of Rage's strength loss), then
  enemies act one at a time in spawn order (left to right). Each enemy's block
  resets at the start of its own turn; its end-of-turn effects (e.g. Ritual)
  trigger after it acts.
- **Damage pipeline:** `damage = floor(floor((base + attacker Strength) × weak) × vuln)`
  where weak = 0.75 if the attacker is Weak, and vuln = 1.5 if the target is
  Vulnerable. Each multiplication floors. Multi-hit cards run the full pipeline
  per hit. Damage hits Block first; leftover Block persists; overflow reduces HP.
- **Duration statuses** (Vulnerable, Weak): decrement by 1 at the end of the
  afflicted creature's turn. Edge-case timing is verified against the StS wiki
  during implementation and locked in by tests.
- **Card flow:** played Attacks/Skills go to the discard pile; Powers are
  consumed (removed for the rest of combat) and their effect persists. When a
  draw is required and the draw pile is empty, the discard pile is shuffled in
  to form the new draw pile. The data model includes an exhaust pile, but no
  Phase 1 card uses it.
- **Targeting:** single-target Attacks require an enemy target (auto-targeted
  when only one enemy is alive); AoE and self-targeted cards take no target.
- **Intents:** each enemy displays its next move before the player's turn —
  attack (with computed final damage, including the enemy's Strength and the
  player's Vulnerable, recomputed when modifiers change), defend, buff, or
  combinations.
- **End of combat:** when all enemies die, combat ends immediately; the
  player's remaining HP carries into the next fight. Player HP ≤ 0 at any
  point ends the run.
- **Energy:** unspent energy is lost at end of turn.

### Status effects in Phase 1

| Status | Effect |
|---|---|
| Strength | +1 damage per stack on each attack hit |
| Vulnerable | Take ×1.5 damage from attacks (duration) |
| Weak | Deal ×0.75 attack damage (duration) |
| Ritual | At end of own turn, gain N Strength |
| Enrage | When the *player* plays a Skill, gain N Strength (enemy-only) |
| Curl Up | First time taking attack damage, gain rolled Block, then expires |
| Strength Down | At end of own turn, lose N Strength (implements Surge of Rage) |

### Cards — 12 designs, exact StS numbers (unupgraded)

Starter deck: 5× Hew, 4× Raise Shield, 1× Skull-Splitter.

| Norse name | StS original | Type | Cost | Effect |
|---|---|---|---|---|
| Hew | Strike | Attack | 1 | Deal 6 damage |
| Raise Shield | Defend | Skill | 1 | Gain 5 Block |
| Skull-Splitter | Bash | Attack | 2 | Deal 8 damage, apply 2 Vulnerable |
| Whirling Axe | Cleave | Attack | 1 | Deal 8 damage to ALL enemies |
| Haft Strike | Pommel Strike | Attack | 1 | Deal 9 damage, draw 1 card |
| Unbowed | Shrug It Off | Skill | 1 | Gain 8 Block, draw 1 card |
| Shield Charge | Iron Wave | Attack | 1 | Deal 5 damage, gain 5 Block |
| Twin Axes | Twin Strike | Attack | 1 | Deal 5 damage twice |
| Rising Fury | Anger | Attack | 0 | Deal 6 damage, add a copy of this card to the discard pile |
| Surge of Rage | Flex | Skill | 0 | Gain 2 Strength; at end of turn lose 2 Strength |
| Berserkergang | Inflame | Power | 1 | Gain 2 Strength |
| Thor's Wrath | Thunderclap | Attack | 1 | Deal 4 damage to ALL enemies, apply 1 Vulnerable to ALL |

Cards are **data, not code**: a `CardSpec` carries cost, type, targeting, and a
list of `Effect` values interpreted by the engine. Adding a card later is a
table entry, not new logic.

### Enemies — 5 species, StS-faithful AI

Intent patterns, probabilities, and no-repeat rules follow the StS wiki
(base difficulty, Ascension 0), rolled on the seeded RNG. HP is rolled
uniformly within range at spawn.

| Norse name | StS original | HP | Behavior |
|---|---|---|---|
| Draugr Chanter | Cultist | 48–54 | Turn 1: chant (gain Ritual 3). Every turn after: attack 6 |
| Grave Wolf | Jaw Worm | 40–44 | Turn 1: Chomp (attack 11). Then weighted random: Chomp 11 / Thrash (attack 7 + 5 Block) / Bellow (+3 Strength, +6 Block), with StS no-repeat rules |
| Barrow Rat | Red Louse | 10–15 | Bite (attack, damage rolled 5–7 at spawn) / Grow (+3 Strength); spawns with Curl Up 3–7 |
| Fen Rat | Green Louse | 11–17 | Bite (attack, rolled 5–7) / Diseased Spittle (apply 2 Weak); spawns with Curl Up 3–7 |
| Forest Troll | Gremlin Nob (elite) | 82–86 | Turn 1: Bellow (gain Enrage 2). Then Rush (attack 14) / Skull Bash (attack 6, apply 2 Vulnerable) per StS pattern |

### The gauntlet — "the Barrow Road"

Phase 1's stand-in for the map: a fixed three-fight sequence.

1. **Fight 1:** Draugr Chanter *or* Grave Wolf — picked from the encounter
   pool by seeded RNG.
2. **Card reward:** choose 1 of 3 distinct cards rolled uniformly from the
   9-card reward pool (no duplicates within one offer; duplicates across
   rewards allowed), or skip.
3. **Fight 2:** Barrow Rat + Fen Rat (pair fight — exercises multi-enemy
   targeting, Weak, Curl Up).
4. **Card reward** (same rules).
5. **Fight 3:** Forest Troll — the elite finale; Enrage punishes skill spam.
6. **Victory screen** with run stats (turns taken, damage dealt/taken, final deck).

No healing between fights. Death at any point → game-over screen → new run
with a fresh seed.

### Determinism

All randomness (shuffles, enemy rolls, HP rolls, rewards) flows through a
single `ChaCha8Rng` seeded per run. The seed is shown on the victory/death
screen, and `helheim --seed <n>` replays a run. Same seed + same actions ⇒
identical outcome, byte for byte.

## Architecture

### Workspace — two crates

```
rogue-like-game/
├── Cargo.toml                 # workspace
├── crates/
│   ├── helheim_core/          # rules engine — NO Bevy dependency
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── cards.rs       # CardId, CardSpec, Effect, card table
│   │       ├── combat.rs      # CombatState, turn engine, damage pipeline
│   │       ├── statuses.rs    # status definitions & timing
│   │       ├── enemies.rs     # EnemyId, specs, intent-pattern AI
│   │       ├── run.rs         # RunState: deck, HP, gauntlet progress, seed
│   │       └── rng.rs         # seeded RNG wrapper
│   └── helheim/               # Bevy 0.18 binary
│       └── src/
│           ├── main.rs        # App wiring, states, window
│           ├── screens/       # one plugin per screen
│           │   ├── menu.rs
│           │   ├── combat.rs
│           │   ├── reward.rs
│           │   └── end.rs     # victory / game over
│           ├── anim.rs        # event queue → sequential presentation
│           └── theme.rs       # colors, layout constants, font handle
└── docs/superpowers/specs/    # this document
```

Dependencies: `helheim_core` → `rand`, `rand_chacha`. `helheim` → `bevy 0.18`,
`helheim_core`. Dev profile uses Bevy's `dynamic_linking` feature and
optimized dependencies (`[profile.dev.package."*"] opt-level = 3`) for
iteration speed.

### Core API

The engine is a synchronous state machine: the shell drives it with actions,
it answers with an ordered list of events (the facts of what happened).

```rust
impl CombatState {
    /// Begin a fight; returns initial events (intents set, opening hand drawn).
    fn new(rng: &mut RunRng, deck: &[CardId], encounter: Encounter) -> (Self, Vec<CombatEvent>);
    /// Apply a player action. Errors on illegal actions; never panics.
    fn apply(&mut self, rng: &mut RunRng, action: Action) -> Result<Vec<CombatEvent>, IllegalAction>;
}

enum Action { PlayCard { hand_index: usize, target: Option<EnemyIndex> }, EndTurn }

enum CombatEvent {
    CardPlayed, CardDrawn, DeckShuffled, DamageDealt { target, amount, blocked },
    BlockGained, StatusApplied, StatusExpired, EnergyChanged, IntentSet,
    EnemyTurn, EnemyDied, PlayerDied, Victory, /* … */
}

enum IllegalAction { NotEnoughEnergy, InvalidTarget, NeedsTarget, NoSuchCard, CombatOver }
```

`RunState` owns the deck, player HP, gauntlet position, RNG, and seed, and
constructs each `CombatState`. Card rewards are a `RunState` operation.

### Bevy shell

- **Screens as `States`:** `Menu → Combat → Reward → (Victory | GameOver)`.
  One plugin per screen: spawn UI on enter, despawn on exit, systems gated
  by `in_state`.
- **The animation queue** (the load-bearing pattern): when the player acts,
  the shell calls `core.apply(...)` — resolution is instant — and pushes the
  returned events into a queue resource. Presentation systems drain the queue
  *sequentially* with short timers (~0.2 s per beat): damage numbers float and
  fade, HP bars tick, intents update. Input is locked while the queue is
  non-empty, so the player watches causality unfold in order.
- **Input:** mouse — click a card; if it needs a target, enter targeting mode
  (click an enemy, Esc cancels); unaffordable cards are grayed out and
  unclickable. Keyboard — `1`–`9`/`0` select card, arrows/Tab cycle targets,
  Enter confirm, `E` end turn.
- **Rendering:** programmer art only — Bevy UI nodes, panels, and text with
  one bundled open-license font. Layout: enemies in a row on the right with
  HP/Block/intent/statuses; player panel on the left; hand as a bottom row of
  card panels; energy bottom-left; draw/discard counters in the corners;
  End Turn bottom-right. Window 1280×720, resizable.

### Error handling

- Core returns `Result` for every action; illegal actions are normal control
  flow (`IllegalAction`), and internal invariants use `debug_assert!`. The
  core library never panics and never performs IO.
- The shell is the first line of defense (graying out illegal plays) but the
  core is the authority — a shell bug cannot corrupt game state.
- Bevy systems tolerate empty queries during state transitions and `warn!`
  rather than unwrap.
- No save system in Phase 1 ⇒ no IO error surface beyond Bevy's own asset
  loading (one font shipped in `assets/`).

## Testing strategy

TDD throughout the core crate (`cargo test -p helheim_core` runs without
compiling Bevy):

- **Unit tests:** damage pipeline ordering and flooring (Strength → Weak →
  Vulnerable), block absorb/persist/expiry, energy gating, draw/reshuffle
  mechanics, hand limit, status tick-down timing, Ritual/Enrage/Curl Up/
  Strength Down triggers, every card's effect list, Power consumption,
  Rising Fury's self-copy.
- **AI conformance tests:** per enemy, simulate many turns on seeded RNG and
  assert pattern constraints (e.g. Grave Wolf never Bellows twice in a row;
  Draugr Chanter always chants exactly once, on turn 1).
- **Integration tests:** scripted action sequences from a fixed seed playing
  the full gauntlet to victory and to death; assert the final state and key
  events.
- **Determinism test:** same seed + same action script ⇒ identical event log.
- **Shell:** kept thin (logic lives in core); a headless `App` smoke test
  (app builds, states transition), everything else via manual play.
- **Standing policy:** `cargo clippy` clean, `cargo fmt` applied.

## Setup (implementation prerequisites)

- Install rustup + stable toolchain (none present on this machine yet).
- `git init` the project (not currently a repository); `.gitignore` for `/target`.
- Scaffold the workspace per the layout above; pin Bevy `0.18`.
- Bundle one open-license font (e.g. Fira Sans) under `assets/fonts/`.

## Definition of done (Phase 1)

1. `cargo run -p helheim` opens a window; a full gauntlet run is playable to
   victory and to death with mouse or keyboard.
2. All core tests pass; clippy is clean.
3. `--seed` reproduces a run deterministically; the seed is shown on end screens.
4. The full-gauntlet integration test exists and passes.
