# Helheim — Combat Card Animations & Type Identity Design

**Status:** Approved design, ready for implementation planning.
**Predecessors:** Phase 1 Combat Core (shipped), [`2026-06-15-map-visual-overhaul-design.md`](2026-06-15-map-visual-overhaul-design.md) (established the world/UI animation patterns and the procedural-icon generator reused here).

## Context

In combat, the hand renders as flat panels — affordable cards grey, unaffordable dim — showing "(cost) Name", description, and a hotkey ([`combat.rs:306-360`](../../../crates/helheim/src/screens/combat.rs#L306-L360)). All cards look alike, and the whole hand is **despawned and rebuilt on every `DisplayState` change**, so cards have no stable identity and nothing animates: a played card just blinks out, a drawn card blinks in. This makes cards hard to tell apart and hard to follow.

This spec gives cards a **per-type visual identity** *and* **motion feedback**, entirely in the presentation layer. `helheim_core` is untouched — it already emits the card-flow events and exposes `CardKind`.

## Goal

Make combat cards easy to identify at a glance and easy to follow in motion:
- **Type identity:** Attack / Skill / Power become visually distinct — a type colour, a type icon (sword / shield / sparkle), and a coloured frame (the "icon-forward" card look).
- **Motion (four animations):** hover lift, play → fly-to-discard, draw → slide-in, and a targeting pulse on the card awaiting an enemy.

Built with Bevy 0.18's `UiTransform` (scale / translate / rotate a UI node without disturbing layout), so the hand stays in the UI.

**Conventions (inherited):** the rules engine stays pure and deterministic; all animation is cosmetic and time-based, never touching core state. Pure presentation helpers are unit-tested.

## Scope

**In scope:** per-type card colour + icon + frame + cost gem (the Option-C look); a reconcile that gives cards stable identity; the four `UiTransform` animations; three generated card-type icons; a `CardFlow` display message so the hand knows exactly what drew/played/discarded; promoting `combat.rs` to a `screens/combat/` module; unit tests for the pure helpers.

**Out of scope (YAGNI):** per-card unique artwork (type icon only); a smooth *reflow* tween when a card leaves (the played card animates out; remaining cards reflow immediately — smoothing is later polish); any change to reward / rest / map screens; any `helheim_core` change; new card mechanics.

---

## 1. Architecture & where it lives

Approach **A** (chosen over a world-space hand and over an overlay-ghost hack): keep the hand in Bevy UI and animate it with `UiTransform`. Two shell changes:

- **[`anim.rs`](../../../crates/helheim/src/anim.rs):** `drain_queue` already pops every `CombatEvent` as it animates the beat. It additionally emits a **`CardFlow` message** for the card-flow events, so the hand renderer knows exactly what happened (essential when the hand holds duplicate `CardId`s — a `Vec` diff can't tell which slot left).

```rust
#[derive(Message, Clone, Copy, Debug)]   // bevy 0.18 buffered-event = Message
pub enum CardFlow {
    Drawn(CardId),       // from CombatEvent::CardDrawn
    Played { slot: usize }, // from CombatEvent::CardPlayed { hand_index }
    Discarded,           // from CombatEvent::HandDiscarded (whole hand)
}
```
Registered via `AnimPlugin` (`app.add_message::<CardFlow>()`); `DeckShuffled` / `CardAddedToDiscard` don't change hand entities and are not surfaced.

- **`combat.rs` → `screens/combat/` module** (it would exceed ~750 lines otherwise). `helheim_core` unchanged.

## 2. Module layout

| File | Responsibility |
|---|---|
| `screens/combat/mod.rs` | `CombatScreenPlugin`, resources, enter/exit, battlefield panels + `Bind`/`sync_texts`, input (`card_click`, `enemy_click`, `keyboard`, `end_turn_button`, `try_play`, `dispatch`), enemy highlight, `post_combat` — the existing non-hand code |
| `screens/combat/hand.rs` | Card visuals (Option-C spawn), the `reconcile_hand` system, the four animation systems, and pure helpers (`kind_color`, slot→offset math) |

`screens/mod.rs` already declares `pub mod combat;`, which resolves to `combat/mod.rs` unchanged.

## 3. Card visuals (Option C, in UI)

Per-type colour from a pure helper:

```rust
pub fn kind_color(kind: CardKind) -> Color {
    match kind {
        CardKind::Attack => theme::ACCENT,                // red  #c73833
        CardKind::Skill  => theme::BLOCK_COLOR,           // blue #6194eb
        CardKind::Power  => theme::ENERGY_COLOR,          // gold #f2c238
    }
}
```

Each card is a `Button` node (~138×178) composed of:
- a **type-coloured frame** — `Node.border` + `BorderColor(kind_color)` + `BorderRadius`;
- a faint large **watermark** `ImageNode` of the type icon (absolute, low alpha) behind the content;
- a **big type icon** `ImageNode` (tinted `kind_color`) near the top;
- a circular **cost gem** (top-left): a small node with `BorderRadius::MAX`, background `kind_color`, dark cost number;
- **name**, **description**, and **hotkey** text (as today).
- **Affordability:** `cost > energy` dims the whole card (reduced alpha on frame/icon/text), matching today's PANEL_DIM behaviour.

**New assets:** `crates/helheim/assets/icons/card_attack.png`, `card_skill.png`, `card_power.png` — white silhouettes (sword / shield / sparkle), generated by extending [`tools/gen_map_icons.py`](../../../tools/gen_map_icons.py) (same supersampled rasterizer) and loaded once into a resource at plugin build, like `MapAssets`. Tinted per type at runtime via `ImageNode` colour.

## 4. Hand reconcile (event-sourced)

Replace `rebuild_hand` (nuke-and-rebuild) with **`reconcile_hand`**, which drains `CardFlow` and maintains persistent card entities. Each card entity carries its current slot:

```rust
#[derive(Component)] pub struct Card { pub slot: usize, pub card: CardId }  // kind = card.spec().kind
#[derive(Component)] pub struct CardEnter { pub timer: Timer }   // draw-in
#[derive(Component)] pub struct CardFlyOut { pub timer: Timer }  // play/discard-out
```

- **`Drawn(card)`** → spawn a card entity at the tail slot (`slot = current card count`), tagged `CardEnter`. (Draw appends in the engine — `apply_event` does `ds.hand.push(card)` — so tail-slot is correct.)
- **`Played { slot }`** → tag that entity `CardFlyOut`, drop its `Button`/make it non-interactive, and decrement `slot` on every card with a higher slot (they shift left).
- **`Discarded`** → tag all card entities `CardFlyOut`.

Card slots mirror `DisplayState.hand` indices (both derive from the same events), so `card_click`/hotkeys dispatch the correct `hand_index` via `try_play(card.slot)`. A separate pass refreshes affordability styling when `DisplayState` changes (energy moved) without respawning.

The opening hand deals in for free: the fight's opening draw events flow through `CardFlow::Drawn` → draw-in animation on combat start.

## 5. Animations (all via `UiTransform`, eased over `Time`)

`UiTransform { translation: Val2, scale: Vec2, rotation: Rot2 }` is applied to the card node; it affects rendering only, not layout.

1. **Hover lift** (`hover_cards`): reads `Interaction`; the hovered card eases `scale → ~1.08` and translates up a few px; others ease back to identity. (Suppressed on cards that are `CardFlyOut`.)
2. **Draw-in** (`animate_enter`): a `CardEnter` card starts at `scale ~0.6`, translated toward the draw pile (bottom-left) with reduced alpha, and eases to identity over ~0.3 s; remove `CardEnter` when done. Multiple draws stagger by slot.
3. **Play → fly-out** (`animate_flyout`): a `CardFlyOut` card is set `position_type: Absolute` (so the row reflows), eases `translation` toward the discard corner (bottom-right) with `scale → ~0.4`, and despawns when its timer finishes.
4. **Targeting pulse** (`pulse_pending`): while `PendingCard(Some(slot))`, the card at that slot pulses (`scale` sine + a brightened border) until the player clicks/confirms an enemy or cancels.

Feel constants (`HOVER_SCALE`, `ENTER_SECS ≈ 0.3`, `FLYOUT_SECS ≈ 0.4`, pulse rate) live at the top of `hand.rs`, tunable in play-test.

## 6. Data flow & lifecycle

- **`OnEnter(Combat)`** (unchanged shell flow): build the battlefield UI + an empty `HandRow`; the queued opening events drain → `CardFlow::Drawn` → cards deal in.
- **Per frame:** `reconcile_hand` drains `CardFlow` (spawn/flag entities); the four animation systems ease `UiTransform`; affordability refresh on `DisplayState` change. Input (`card_click`/`keyboard`/`enemy_click`/`end_turn`) runs only while `queue_empty` — unchanged — so animation and input never fight.
- **Play:** click/hotkey → `try_play(slot)` → `dispatch` → core events → `drain_queue` animates the beat and emits `CardFlow::Played { slot }` → `animate_flyout`.
- **`OnExit(Combat)`:** despawn `CombatRoot`; clear the queue; drop `DisplayState`/`PendingCard` (as today). Card entities are children of the hand row under `CombatRoot`, so they despawn with it.

## 7. Error handling & determinism

- Input stays gated by `queue_empty`; `try_play` still rejects unaffordable/invalid plays before dispatch, and `dispatch` `warn!`s on a core `Err` rather than panicking (unchanged).
- A card mid-fly-out is non-interactive (no `Button`), so it can't be re-clicked; remaining cards keep correct slots.
- Icons not yet loaded: the `ImageNode` is briefly invisible (frame/gem/text still show) — acceptable.
- Animations read only `Time` and display state and mutate only `UiTransform`/colours — they never touch `RunRng` or core state, so runs stay deterministic. The reconcile derives purely from `CardFlow`, which derives from core events.

## 8. Testing

**Unit tests (pure helpers):**
- `kind_color`: each `CardKind` maps to its themed colour; the three are distinct.
- `card_flow_from_event`: `CombatEvent::CardDrawn → Drawn`, `CardPlayed { hand_index } → Played { slot }`, `HandDiscarded → Discarded`, and that `DeckShuffled`/`CardAddedToDiscard` produce nothing.
- slot→offset math (draw-pile / discard-pile target offsets) returns the expected direction/magnitude.

**Headless test:** stepping the combat app through a played card produces a `CardFlow::Played` message (mirrors the existing menu/map smoke tests).

**Unchanged:** all `helheim_core` tests and existing combat shell tests.

**Manual play-test checklist:** cards show per-type colour + icon + frame; affordable vs dim; hover lifts the card; playing a card flies it to the discard; draws slide in from the draw pile (opening hand deals in); the targeting card pulses; keyboard play (1–9/0) and click both animate; combat still resolves and routes correctly.

## 9. Non-goals (YAGNI)

No per-card unique art (type icon only). No smooth reflow tween when a card leaves (immediate reflow; later polish). No reward/rest/map changes. No `helheim_core` changes. No new card mechanics.
