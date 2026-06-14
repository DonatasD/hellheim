# Helheim — Act 1 Map Foundation (Phase 2, Spec 1) Design

**Status:** Approved design, ready for implementation planning.
**Predecessors:** [`2026-06-10-helheim-design.md`](2026-06-10-helheim-design.md) (overall design), Phase 1 Combat Core (shipped).

## Context: Phase 2 decomposition

Phase 2 ("Act 1 run loop") is too large for one spec. It is built as an ordered
sequence of specs, each its own spec → plan → build cycle, each independently
playable:

| # | Spec | Contents | Status |
|---|------|----------|--------|
| **1** | **Act 1 Map Foundation** | Branching node map + navigation; Monster/Elite/Boss routing into combat; Rest (heal) + Treasure (card); small new bestiary + a boss | **this spec** |
| 2 | Gold & Shops | Gold from combat; Shop nodes (buy cards, pay-to-remove) | follow-up |
| 3 | Card Upgrades | Upgraded card forms; applied at Rest; stocked in Shops | follow-up |
| 4 | Events | Event nodes with choices and outcomes | follow-up |
| 5 | Save & Continue | Serialize/resume an in-progress run | follow-up |

This spec is the **skeleton** every later piece plugs into.

## Goal

Replace Phase 1's fixed three-fight gauntlet with a Slay-the-Spire-style
**branching node map** for Act 1: a deterministic ~15-floor graph the player
climbs by choosing connected nodes, fighting through Monster/Elite encounters
and a boss, resting and collecting treasure along the way. The rules engine
stays pure, deterministic, and fully unit-tested (`helheim_core`); the Bevy
shell renders the map and routes node selections.

**Conventions (inherited from Phase 1):** Slay the Spire wiki numbers are
authoritative; integer math (truncating division); everything flows from one
seeded `RunRng` so a seed reproduces a whole run. Tests encode the numbers.

## Scope

**In scope:** branching map generation (StS-faithful), map navigation, the map
screen, routing Monster/Elite/Boss nodes into the existing combat engine, Rest
(heal) and Treasure (bonus card) nodes, a small new bestiary (3 normal species,
2 elites, 1 boss) and the encounter table.

**Out of scope (later specs / phases):** Shop nodes & gold (spec 2), card
upgrades (spec 3), Event nodes (spec 4), save/continue (spec 5), relics &
potions (Phase 3), Act 2+ (later). Max-HP increases. The Lagavulin "sleep" and
Guardian "mode-shift" enemy mechanics (would need conditional-intent logic).

---

## 1. Architecture & core data model

The map lives in `helheim_core`, generated deterministically from the run seed
via `RunRng` — same discipline as combat. The Bevy shell reads it directly (like
it reads `CombatState`) and sends node selections; combats still play through the
existing `CombatEvent` replay / beat system.

New core module `map.rs`:

```rust
pub enum NodeKind { Monster, Elite, Rest, Treasure, Boss }   // Shop, Event added in later specs
pub struct NodeId { pub floor: u8, pub col: u8 }             // floors 1..=15, boss = 16
pub struct MapNode { pub id: NodeId, pub kind: NodeKind, pub next: Vec<NodeId> }  // edges go upward
pub struct MapGraph { /* nodes indexed by floor */ }
impl MapGraph {
    pub fn generate(rng: &mut RunRng) -> Self;
    pub fn floor1(&self) -> Vec<NodeId>;            // starting choices
    pub fn node(&self, id: NodeId) -> &MapNode;
}
```

`run.rs` refactor — `Stage` stops being the linear `Fight(1..=3)` and becomes
map-driven:

```rust
pub enum Stage { ChoosingNode, Reward { offer: [CardId; 3], source: RewardSource }, Victory, Defeat }
pub enum RewardSource { Combat, Treasure }          // both show the 3-card screen; return to map after

pub struct RunState {
    // existing: seed, rng, master_deck, hp, max_hp, stats
    map: MapGraph,
    position: Option<NodeId>,        // None = haven't stepped onto floor 1 yet
    pub stage: Stage,
    pub combat: Option<CombatState>,
}

impl RunState {
    pub fn available_nodes(&self) -> Vec<NodeId>;                       // legal next moves
    pub fn enter_node(&mut self, id: NodeId) -> Result<Vec<CombatEvent>, RunError>;
    pub fn choose_reward(&mut self, pick: Option<usize>) -> Result<(), RunError>;  // → ChoosingNode
}
```

`enter_node` validates the move (must be in `available_nodes`), then routes by
`kind`: Monster/Elite/Boss build a `CombatState` from the encounter table
(`CombatState::new` already accepts a multi-enemy `&[Species]`); Rest applies a
heal; Treasure produces a card-choice reward. After a combat-node win the player
gets the card reward, then returns to `ChoosingNode`; **boss win → `Victory`;
any combat loss → `Defeat`.** The whole map is reproducible from the seed.

## 2. Map generation (StS-faithful)

**Structure:**
- 15 floors + a single **boss node** on top (floor 16); up to 7 columns wide.
- **6 paths** walked bottom→top: each starts at a random floor-1 column and steps
  into an adjacent column (`c-1`/`c`/`c+1`) with **edge-crossing avoidance** (a
  step may not cross a neighbour's edge). Paths share nodes where they coincide;
  **all floor-15 nodes connect to the boss.**
- Guarantee **≥2 distinct floor-1 nodes** (re-roll starts if they collapse to one).

**Node-type assignment** (after structure is built):
- **Fixed:** floor 1 = Monster, floor 9 = Treasure, floor 15 = Rest, floor 16 = Boss.
- **Constraints:** no Elite or Rest before floor 6; no Rest on floor 14; a node may
  not be the same special type as a node it connects down to (no consecutive
  same-special on a path).
- **Weights** for remaining nodes: StS uses Monster 45 / Event 22 / Elite 16 /
  Rest 12 / Shop 3. Event and Shop are deferred (specs 4 & 2), so their 25% folds
  into Monster for now → **Monster ~72% / Elite 16% / Rest 12%**, preserving StS's
  elite/rest density exactly. Their slices are restored to faithful values when
  those specs land.

## 3. Node behaviors & rewards

| Node | Behavior | Reward |
|------|----------|--------|
| ⚔️ Monster | Normal combat (encounter by floor) | 3-card choice (Phase 1 reward) |
| 💀 Elite | Tougher combat (elite encounter) | 3-card choice (distinguished by difficulty, not loot, this spec) |
| 👑 Boss | Boss combat (floor 16) | Win → **Victory** (Act 1 complete) |
| 🔥 Rest | Heal `floor(0.30 · max_hp)` = 24 at 80, capped at max | resolves → back to map |
| 💰 Treasure | — | A bonus 3-card choice |

- The **card reward reuses Phase 1's `roll_offer`/`choose_reward`** machinery
  (`RewardSource` only differs in where it returns to — always back to the map).
- HP carries across the whole map (already does); **max HP fixed at 80**.
- StS elites/bosses/treasure normally drop gold & relics — both deferred, so
  Elite/Treasure grant a card and the boss simply completes the act this spec.

## 4. Bestiary & encounter table

Norse barrow theme; StS analogues give the numbers; **only existing mechanics are
used** (attack, block, Strength, Ritual, Weak, Vulnerable, multi-hit). Numbers
below are the design target; the implementation pins exact values (wiki-authoritative
where an analogue exists) and tests encode them.

**New normal species:**

| Name | StS analogue | HP | Moves |
|------|--------------|----|-------|
| Draugr Warrior | Blue Slaver | 46–50 | Stab (attack 12); Rend (attack 8 + apply Weak 1) |
| Mire Crawler | Fungi Beast | 22–28 | Bite (attack 6); Fester (gain Strength 4) |
| Hrafn (carrion crow) | custom flier | 30–34 | Peck (multi-hit 2×4); Screech (attack 5 + apply Vulnerable 1) |

**New elites** (join the existing **ForestTroll**, 82–86 HP):

| Name | StS analogue | HP | Moves |
|------|--------------|----|-------|
| Barrow Wight | Lagavulin | 85–90 | Maul (attack 18); Soul Drain (apply Strength −2 to player) |
| Draugr Warlord | Gremlin Leader | 86–90 | War-Chant (gain Ritual 2); Cleave (multi-hit 2×8) |

**New boss:**

| Name | HP | Moves (rotation) |
|------|----|------------------|
| The Mound Jarl | ~150 | Crushing Blow (attack 22); Grave Cleave (multi-hit 3×6); Dread Roar (gain Strength 3 + apply Vulnerable 2); Bulwark (gain Block 18 + attack 10) |

**Encounter pools** (deterministic via `RunRng`, no-immediate-repeat):
- **Weak pool** (monster nodes on floors 1–3): `[BarrowRat]`, `[FenRat]`, `[GraveWolf]`, `[DraugrChanter]`, `[BarrowRat, FenRat]`.
- **Strong pool** (monster nodes on floors 4+): `[Draugr Warrior]`, `[Mire Crawler ×2]`, `[Hrafn, GraveWolf]`, `[DraugrChanter, FenRat]`, `[BarrowRat ×2, FenRat]`, `[Draugr Warrior, Mire Crawler]`.
- **Elite pool**: `[ForestTroll]`, `[Barrow Wight]`, `[Draugr Warlord]`.
- **Boss**: `[The Mound Jarl]`.

The floors 1–3 / 4+ split approximates StS's "weak pool for the first three
combats, then strong pool."

**Likely small core addition:** letting an *enemy* attack hit multiple times.
The player-facing `IntentKind::Attack { hits }` already exists; the enemy-attack
resolution path may currently assume single hits and need to honour `hits`.

## 5. Shell (Bevy): map screen & navigation

- New `AppState::Map`: a scrollable vertical branching graph; top bar shows
  **floor + HP**; the current node is marked, legal next moves glow, everything
  else is dimmed.
- **Input mirrors combat**: click a glowing node, or `↑/←/→` to move the selection
  and `Enter` to confirm.
- **New shell pieces**: `MapPlugin` (the map screen) and a tiny `RestPlugin`.
  Selecting a Rest node **applies the heal immediately in core** (`enter_node`);
  the Rest screen is an acknowledgement ("rest by the fire — healed +24") with a
  Continue button back to the map — not a gameplay gate (spec 3 adds the
  card-upgrade choice here). **Treasure reuses the existing reward screen.** The
  combat & reward screens are unchanged except that reward now returns to
  `AppState::Map` instead of the next linear fight.
- Flow: Menu → Begin (generate map) → Map → {Combat→Reward / Rest / Treasure→Reward}
  → back to Map → … → Boss → Victory; any combat loss → GameOver.

## 6. Testing & determinism

The engine stays pure and fully testable.

**Core `map.rs` (across seeds 0..100):**
- *Structure:* 15 floors + boss; ≥2 distinct floor-1 starts; edges only go up one
  floor to adjacent columns; **no edge crossings**; every floor-1 node reaches the
  boss; no orphan nodes.
- *Placement:* floor 1 all Monster, floor 9 Treasure, floor 15 Rest, floor 16 Boss;
  no Elite/Rest below floor 6; no Rest on floor 14; no consecutive same-special.
- *Determinism:* same seed → identical graph. *Weight bands:* Elite/Rest
  frequencies land in expected ranges (enemy-AI-distribution test style).

**Navigation `run.rs`:** `available_nodes()` returns only connected nodes (floor-1
set at start); `enter_node` routes by kind; Rest heals 30% capped at max HP;
illegal moves (unconnected node, entering during combat) rejected.

**Bestiary `enemies.rs`:** new species data + `roll_move` no-repeat patterns
(existing test style); encounter-pool selection deterministic, no-immediate-repeat,
first monster fight from the weak pool.

**Integration `tests/`:** extend the policy-bot to drive a **full map run** —
navigate nodes (fixed policy, e.g. leftmost reachable), fight, take
rewards/rest/treasure — from Menu to Victory/Defeat across many seeds. Assert every
seed terminates with consistent state, and **same-seed runs are byte-for-byte
identical** (the determinism fingerprint extended to include map choices).

**Shell:** verified by play-test + a startup smoke-check (watching for Bevy
API/ordering gotchas, as in Phase 1).

---

## Definition of done

1. `cargo run -p helheim` opens to a generated Act 1 map; a full run is playable
   from the first floor to the boss (Victory) and to death (GameOver), with mouse
   or keyboard.
2. Map generation is StS-faithful per §2; all core invariants and placement rules
   hold across seeds; the new bestiary and encounters are in play.
3. Rest heals, Treasure grants a card, rewards return to the map, boss win wins
   the act.
4. All core + integration tests pass; clippy clean (`-D warnings`); fmt applied.
5. `--seed` reproduces a whole run (map + encounters + combat) deterministically.
6. The full-map gauntlet integration test exists and passes.
