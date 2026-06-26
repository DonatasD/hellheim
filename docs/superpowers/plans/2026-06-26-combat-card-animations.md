# Combat Card Animations & Type Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give combat cards a per-type visual identity (color + icon + frame) and four `UiTransform` motions (hover lift, play→fly-to-discard, draw→slide-in, targeting pulse), reworking the hand from nuke-rebuild into an event-sourced reconcile.

**Architecture:** Stay in Bevy UI (Approach A). `anim.rs`'s beat drainer emits a new `CardFlow` message describing each draw/play/discard; the combat hand consumes it to keep persistent, individually-animated card entities. `helheim_core` is untouched. `combat.rs` is promoted to a `screens/combat/` module (`mod.rs` + `hand.rs`).

**Tech Stack:** Rust, Bevy 0.18 (`UiTransform` for scale/translate without layout impact, `BorderColor`/`BorderRadius`/`ImageNode` for the card frame/icon/gem, `Message`/`MessageReader`/`MessageWriter` for `CardFlow`), `helheim_core` (read-only).

**Spec:** [`docs/superpowers/specs/2026-06-26-combat-card-animations-design.md`](../specs/2026-06-26-combat-card-animations-design.md)

## Global Constraints

- Bevy **0.18** APIs: `MessageWriter::write(msg)` / `MessageReader::read()` (not `EventReader`); `app.add_message::<T>()` to register; `UiTransform { translation: Val2, scale: Vec2, rotation: Rot2 }` with `Val2::px(x,y)`, `UiTransform::default()` (identity, scale ONE); `BorderColor::all(color)`; `ImageNode { color, ..ImageNode::new(handle) }`; `cameras.single_mut()` returns `Result`.
- `helheim_core` is **never modified**.
- The project lint bar is `cargo clippy --workspace --all-targets -- -D warnings` (enforced in the final task). A pre-existing third-party `block v0.1.6` future-incompat note is NOT our code — ignore it.
- Bevy systems with >7 params need `#[allow(clippy::too_many_arguments)]`.
- All animation is cosmetic/time-based; never read or mutate `RunRng` or core run state.
- Per-type colors come from `theme`: Attack `ACCENT`, Skill `BLOCK_COLOR`, Power `ENERGY_COLOR`.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/helheim/src/anim.rs` (modify) | Add `CardFlow` message + pure `card_flow()` mapping; register + emit it in `drain_queue` |
| `crates/helheim/src/screens/combat.rs` → `screens/combat/mod.rs` (move) | Plugin, panels, `Bind`/`sync_texts`, input (`card_click`/`keyboard`/`enemy_click`/`end_turn`/`try_play`/`dispatch`), enemy highlight, `post_combat` |
| `crates/helheim/src/screens/combat/hand.rs` (create) | `kind_color`, `CardAssets`, card components, `spawn_card`, `reconcile_hand`, `refresh_affordability`, the four animation systems, pure helpers |
| `crates/helheim/assets/icons/card_{attack,skill,power}.png` (create) | White silhouette type icons |
| `tools/gen_map_icons.py` (modify) | Also emit the three card icons |

---

## Task 1: `CardFlow` message + emission (anim.rs)

**Files:**
- Modify: `crates/helheim/src/anim.rs`

**Interfaces:**
- Produces: `pub enum CardFlow { Drawn(CardId), Played { slot: usize }, Discarded }` (derives `Message, Clone, Copy, Debug, PartialEq`); `pub fn card_flow(ev: &CombatEvent) -> Option<CardFlow>`. `AnimPlugin` registers `add_message::<CardFlow>()` and `drain_queue` writes one per matching event.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `anim.rs`:

```rust
    #[test]
    fn card_flow_maps_only_hand_events() {
        assert_eq!(card_flow(&CombatEvent::CardDrawn { card: CardId::Hew }), Some(CardFlow::Drawn(CardId::Hew)));
        assert_eq!(
            card_flow(&CombatEvent::CardPlayed { card: CardId::Hew, hand_index: 2 }),
            Some(CardFlow::Played { slot: 2 })
        );
        assert_eq!(card_flow(&CombatEvent::HandDiscarded), Some(CardFlow::Discarded));
        assert_eq!(card_flow(&CombatEvent::DeckShuffled), None);
        assert_eq!(card_flow(&CombatEvent::CardAddedToDiscard { card: CardId::Hew }), None);
    }
```

- [ ] **Step 2: Run it, expect failure**

Run: `cargo test -p helheim card_flow_maps_only_hand_events`
Expected: FAIL to compile (`CardFlow` / `card_flow` not found).

- [ ] **Step 3: Add the type and pure mapping**

Near the top of `anim.rs` (after the existing `use` lines), add:

```rust
/// A hand-level change derived from the combat event stream — lets the hand
/// renderer animate exactly the card that drew/played/discarded.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum CardFlow {
    Drawn(CardId),
    Played { slot: usize },
    Discarded,
}

/// Pure: map a core combat event to a hand-flow message, if it affects the hand.
pub fn card_flow(ev: &CombatEvent) -> Option<CardFlow> {
    match *ev {
        CombatEvent::CardDrawn { card } => Some(CardFlow::Drawn(card)),
        CombatEvent::CardPlayed { hand_index, .. } => Some(CardFlow::Played { slot: hand_index }),
        CombatEvent::HandDiscarded => Some(CardFlow::Discarded),
        _ => None,
    }
}
```

(`Message` is in the Bevy prelude, already glob-imported. `CardId` and `CombatEvent` are already imported in `anim.rs`.)

- [ ] **Step 4: Run the test, expect pass**

Run: `cargo test -p helheim card_flow_maps_only_hand_events`
Expected: PASS.

- [ ] **Step 5: Register and emit the message**

In `AnimPlugin::build`, add the registration (alongside the existing `init_resource`/`insert_resource` calls):

```rust
        app.add_message::<CardFlow>();
```

In `drain_queue`, add a writer parameter and emit inside the pop loop. Change the signature to add:

```rust
    mut flow: MessageWriter<CardFlow>,
```

and inside `while let Some(ev) = queue.0.pop_front() {` — right after the `let beat = ...;` line and before `apply_event(...)` — add:

```rust
        if let Some(f) = card_flow(&ev) {
            flow.write(f);
        }
```

- [ ] **Step 6: Build + full anim tests**

Run: `cargo test -p helheim --lib anim`
Expected: PASS (existing anim tests + the new one). If `MessageWriter`/`add_message` names differ, fix per the compiler (0.18 uses these).

- [ ] **Step 7: Commit**

```bash
git add crates/helheim/src/anim.rs
git commit -m "feat(combat): emit CardFlow messages from the beat drainer"
```

---

## Task 2: Card-type icons (sword / shield / sparkle)

**Files:**
- Modify: `tools/gen_map_icons.py`
- Create: `crates/helheim/assets/icons/card_attack.png`, `card_skill.png`, `card_power.png`

**Interfaces:**
- Produces: three 128×128 white-silhouette PNGs at the paths above.

- [ ] **Step 1: Add three icon builders**

In `tools/gen_map_icons.py`, after the existing `icon_boss` function, add:

```python
def icon_sword():
    """Attack — an upright sword."""
    b = blank()
    fill_poly(b, [(50, 10), (56, 22), (56, 60), (44, 60), (44, 22)], 1)  # blade
    fill_poly(b, rect(34, 60, 66, 67), 1)                                # crossguard
    fill_poly(b, rect(46, 67, 54, 84), 1)                                # grip
    fill_circle(b, 50, 87, 5, 1)                                         # pommel
    return b


def icon_shield():
    """Skill — a shield."""
    b = blank()
    fill_poly(b, [(50, 11), (80, 21), (80, 45), (69, 71), (50, 87),
                  (31, 71), (20, 45), (20, 21)], 1)
    return b


def icon_sparkle():
    """Power — a four-point star."""
    b = blank()
    fill_poly(b, [(50, 7), (57, 43), (93, 50), (57, 57), (50, 93),
                  (43, 57), (7, 50), (43, 43)], 1)
    return b
```

- [ ] **Step 2: Emit them in `main()`**

In `tools/gen_map_icons.py`'s `main()`, after the existing `for name, fn in order:` loop, add a second group:

```python
    card_order = [("card_attack", icon_sword), ("card_skill", icon_shield), ("card_power", icon_sparkle)]
    for name, fn in card_order:
        buf = fn()
        bufs.append(buf)
        write_rgba(os.path.join(icons_dir, name + ".png"), buf)
        print("wrote", name + ".png")
```

(Place it before the `target = ...` contact-sheet block so the three new icons also appear in `target/icon-verify.png`.)

- [ ] **Step 3: Generate and verify dimensions**

Run: `python3 tools/gen_map_icons.py`
Then: `for n in card_attack card_skill card_power; do sips -g pixelWidth -g pixelHeight crates/helheim/assets/icons/$n.png | grep pixel; done`
Expected: each reports `pixelWidth: 128`, `pixelHeight: 128`. (The implementer/controller should also open `target/icon-verify.png` to confirm the shapes read as sword / shield / star.)

- [ ] **Step 4: Commit**

```bash
git add tools/gen_map_icons.py crates/helheim/assets/icons/card_attack.png crates/helheim/assets/icons/card_skill.png crates/helheim/assets/icons/card_power.png
git commit -m "assets(combat): sword/shield/sparkle card-type icons"
```

---

## Task 3: `screens/combat/` module + `kind_color` + `CardAssets`

**Files:**
- Move: `crates/helheim/src/screens/combat.rs` → `crates/helheim/src/screens/combat/mod.rs`
- Create: `crates/helheim/src/screens/combat/hand.rs`

**Interfaces:**
- Consumes: `theme::{ACCENT, BLOCK_COLOR, ENERGY_COLOR}`.
- Produces: `hand::kind_color(CardKind) -> Color`; `hand::CardAssets` resource with `for_kind(CardKind) -> Handle<Image>`, inserted at plugin build.

- [ ] **Step 1: Move the file**

```bash
git mv crates/helheim/src/screens/combat.rs crates/helheim/src/screens/combat/mod.rs
```

- [ ] **Step 2: Create `hand.rs` with the pure helper + asset resource + failing test**

Create `crates/helheim/src/screens/combat/hand.rs`:

```rust
//! Combat hand: card visuals, the event-sourced reconcile, and animations.
use bevy::prelude::*;
use helheim_core::cards::CardKind;

use crate::theme;

/// Per-type accent: Attack red, Skill blue, Power gold.
pub fn kind_color(kind: CardKind) -> Color {
    match kind {
        CardKind::Attack => theme::ACCENT,
        CardKind::Skill => theme::BLOCK_COLOR,
        CardKind::Power => theme::ENERGY_COLOR,
    }
}

/// Card-type icon textures, loaded once and tinted per kind at spawn.
#[derive(Resource)]
pub struct CardAssets {
    attack: Handle<Image>,
    skill: Handle<Image>,
    power: Handle<Image>,
}

impl CardAssets {
    pub fn load(server: &AssetServer) -> Self {
        CardAssets {
            attack: server.load("icons/card_attack.png"),
            skill: server.load("icons/card_skill.png"),
            power: server.load("icons/card_power.png"),
        }
    }
    pub fn for_kind(&self, kind: CardKind) -> Handle<Image> {
        match kind {
            CardKind::Attack => self.attack.clone(),
            CardKind::Skill => self.skill.clone(),
            CardKind::Power => self.power.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_color_is_distinct_per_type() {
        assert_eq!(kind_color(CardKind::Attack), theme::ACCENT);
        assert_ne!(kind_color(CardKind::Attack), kind_color(CardKind::Skill));
        assert_ne!(kind_color(CardKind::Skill), kind_color(CardKind::Power));
    }
}
```

- [ ] **Step 3: Wire the module + load the assets at build time**

In `screens/combat/mod.rs`, add at the top (after the existing `use` lines):

```rust
mod hand;
```

In `CombatScreenPlugin::build`, before the `.add_systems` calls, add:

```rust
        let card_assets = hand::CardAssets::load(app.world().resource::<AssetServer>());
        app.insert_resource(card_assets);
```

- [ ] **Step 4: Add a headless smoke test**

Add to `screens/combat/mod.rs` a test module mirroring the map's asset test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemePlugin;
    use crate::{CliSeed, Session};
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use helheim_core::run::RunState;

    #[test]
    fn card_assets_load_at_plugin_build() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));
        app.init_asset::<bevy::text::Font>();
        app.init_asset::<Image>();
        app.init_state::<AppState>();
        app.insert_resource(CliSeed(Some(0)));
        app.insert_resource(Session { run: RunState::new(0) });
        app.add_plugins((ThemePlugin, crate::anim::AnimPlugin, ThemePlugin, CombatScreenPlugin));
        app.update();
        assert!(app.world().contains_resource::<hand::CardAssets>());
    }
}
```

**Note:** if `CombatScreenPlugin` depends on resources from other plugins at build (it shouldn't beyond `AssetServer`), trim the plugin tuple to what compiles — the map's `map_assets_load_at_plugin_build` is the reference. Remove the duplicate `ThemePlugin` if the compiler complains about double-registration; keep one `ThemePlugin` (it provides `UiFont`) + `AnimPlugin` (provides `EventQueue`/`PendingEvents` the combat plugin may touch).

- [ ] **Step 5: Build + tests**

Run: `cargo test -p helheim` then `cargo build -p helheim`
Expected: compiles; `kind_color_is_distinct_per_type` and `card_assets_load_at_plugin_build` pass; existing tests still green. (Many dead_code warnings for not-yet-used items are fine.)

- [ ] **Step 6: Commit**

```bash
git add crates/helheim/src/screens/combat/
git commit -m "refactor(combat): promote to module; add kind_color + CardAssets"
```

---

## Task 4: Option-C card visuals (`spawn_card`)

Replace the flat card body with the type-identity card, still via the existing rebuild-on-change lifecycle (reconcile comes in Task 5). Introduces the `Card` component and switches input to read it.

**Files:**
- Modify: `crates/helheim/src/screens/combat/hand.rs`
- Modify: `crates/helheim/src/screens/combat/mod.rs`

**Interfaces:**
- Consumes: `kind_color`, `CardAssets`, `theme::{text, UiFont, PANEL, TEXT, TEXT_DIM}`, `DisplayState`, `HandRow`.
- Produces: `#[derive(Component)] pub struct Card { pub slot: usize, pub card: CardId }`; `pub fn rebuild_hand(...)` (moved here); `CardScrim` marker. Input in `mod.rs` reads `&Card` (not `CardButton`).

- [ ] **Step 1: Move `HandRow` + `rebuild_hand` ownership and add the card components/visuals**

In `mod.rs`, the `HandRow` component and `CardButton` component currently live there. Keep `HandRow` defined in `mod.rs` (the spawn UI uses it) but make it `pub(crate)` so `hand.rs` can query it: change `#[derive(Component)] struct HandRow;` to `#[derive(Component)] pub(crate) struct HandRow;`. Delete the `#[derive(Component)] struct CardButton(usize);` line and the old `rebuild_hand` function from `mod.rs` (it moves to `hand.rs`).

In `hand.rs`, add the imports and the card pieces:

```rust
use helheim_core::cards::CardId;

use super::HandRow;
use crate::anim::DisplayState;
use crate::theme::UiFont;

#[derive(Component)]
pub struct Card {
    pub slot: usize,
    pub card: CardId,
}

/// Full-card dark overlay; alpha rises when the card is unaffordable.
#[derive(Component)]
pub struct CardScrim;

const CARD_W: f32 = 138.0;
const CARD_H: f32 = 178.0;

/// Build one Option-C card entity (type frame + icon + watermark + cost gem) and
/// return it. Caller parents it into the hand row.
pub fn spawn_card(
    commands: &mut Commands,
    font: &UiFont,
    assets: &CardAssets,
    card: CardId,
    slot: usize,
    energy: u32,
) -> Entity {
    let spec = card.spec();
    let col = kind_color(spec.kind);
    let icon = assets.for_kind(spec.kind);
    let unaffordable = spec.cost > energy;
    let hot = if slot < 9 { format!("[{}]", slot + 1) } else { "[0]".into() };

    commands
        .spawn((
            Card { slot, card },
            Button,
            UiTransform::default(),
            Node {
                width: Val::Px(CARD_W),
                height: Val::Px(CARD_H),
                border: UiRect::all(Val::Px(2.5)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(9.)),
                ..default()
            },
            BorderColor::all(col),
            BorderRadius::all(Val::Px(10.)),
            BackgroundColor(theme::PANEL),
        ))
        .with_children(|c| {
            // faint watermark icon
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(14.),
                    top: Val::Px(40.),
                    width: Val::Px(108.),
                    height: Val::Px(108.),
                    ..default()
                },
                ImageNode { color: col.with_alpha(0.10), ..ImageNode::new(icon.clone()) },
            ));
            // cost gem
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(6.),
                    top: Val::Px(6.),
                    width: Val::Px(24.),
                    height: Val::Px(24.),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(col),
                BorderRadius::MAX,
            ))
            .with_children(|g| {
                g.spawn(theme::text(font, format!("{}", spec.cost), 14., Color::srgb(0.06, 0.06, 0.08)));
            });
            // big type icon
            c.spawn((
                Node { width: Val::Px(50.), height: Val::Px(50.), margin: UiRect::top(Val::Px(16.)), ..default() },
                ImageNode { color: col, ..ImageNode::new(icon) },
            ));
            // name / text / hotkey
            c.spawn(theme::text(font, spec.name, 14., theme::TEXT));
            c.spawn(theme::text(font, spec.text, 11.5, theme::TEXT_DIM));
            c.spawn(theme::text(font, hot, 11., theme::TEXT_DIM));
            // affordability scrim (covers the whole card)
            c.spawn((
                CardScrim,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.),
                    top: Val::Px(0.),
                    width: Val::Px(CARD_W),
                    height: Val::Px(CARD_H),
                    ..default()
                },
                BorderRadius::all(Val::Px(10.)),
                BackgroundColor(Color::srgb(0.04, 0.04, 0.06).with_alpha(if unaffordable { 0.5 } else { 0.0 })),
            ));
        })
        .id()
}

/// Rebuild the whole hand on display change (replaced by reconcile in a later task).
pub fn rebuild_hand(
    mut commands: Commands,
    ds: Res<DisplayState>,
    font: Res<UiFont>,
    assets: Res<CardAssets>,
    row: Query<Entity, With<HandRow>>,
    existing: Query<Entity, With<Card>>,
) {
    if !ds.is_changed() {
        return;
    }
    let Ok(row) = row.single() else { return };
    for e in &existing {
        commands.entity(e).despawn();
    }
    for (i, card) in ds.hand.iter().enumerate() {
        let e = spawn_card(&mut commands, &font, &assets, *card, i, ds.energy);
        commands.entity(row).add_child(e);
    }
}
```

- [ ] **Step 2: Point the plugin + input at the new symbols**

In `mod.rs`:
- In the `Update` systems tuple, change `rebuild_hand` to `hand::rebuild_hand`.
- In `card_click`, change the query from `Query<(&Interaction, &CardButton), Changed<Interaction>>` to `Query<(&Interaction, &hand::Card), Changed<Interaction>>`, and use `card.slot` where it used `button.0`:

```rust
fn card_click(
    ds: Res<DisplayState>,
    mut pending: ResMut<PendingCard>,
    mut cursor: ResMut<TargetCursor>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    cards: Query<(&Interaction, &hand::Card), Changed<Interaction>>,
) {
    for (interaction, card) in &cards {
        if *interaction == Interaction::Pressed {
            try_play(card.slot, &ds, &mut pending, &mut cursor, &mut session, &mut queue);
        }
    }
}
```

(`keyboard` already plays by digit index — no change. Remove the now-unused `CardButton` references everywhere; `cargo build` will point them out.)

- [ ] **Step 3: Build**

Run: `cargo build -p helheim`
Expected: compiles. Volatile spots: `BorderColor::all`, `BorderRadius::MAX`/`::all`, `ImageNode { color, ..ImageNode::new(h) }`, `UiTransform::default()`, `with_alpha` — all confirmed present in 0.18; adjust only if the compiler objects.

- [ ] **Step 4: Manual checkpoint — the new look (do NOT `cargo run` in CI; this is the human's check)**

Run `cargo run -p helheim`, start a fight. Expect: cards now have a type-colored frame + cost gem, a big sword/shield/sparkle icon + faint watermark, and unaffordable cards are darkened by the scrim. (No animation yet — that's Tasks 6–9.)

- [ ] **Step 5: Commit**

```bash
git add crates/helheim/src/screens/combat/
git commit -m "feat(combat): type-identity card visuals (frame, icon, cost gem)"
```

---

## Task 5: Event-sourced reconcile + affordability refresh

Replace `rebuild_hand` (nuke-on-change) with a `reconcile_hand` that consumes `CardFlow`, keeping persistent card entities, plus a `refresh_affordability` system.

**Files:**
- Modify: `crates/helheim/src/screens/combat/hand.rs`
- Modify: `crates/helheim/src/screens/combat/mod.rs`

**Interfaces:**
- Consumes: `crate::anim::CardFlow`, `DisplayState`, `CardAssets`, `HandRow`, `spawn_card`, `Card`, `CardScrim`.
- Produces: `pub fn reconcile_hand(...)`, `pub fn refresh_affordability(...)`, `#[derive(Component)] pub struct CardEnter { pub timer: Timer }`, `#[derive(Component)] pub struct CardFlyOut { pub timer: Timer }`.

- [ ] **Step 1: Add the animation-marker components and the reconcile/affordability systems**

In `hand.rs`, add the markers near `Card`:

```rust
/// Draw-in animation in progress.
#[derive(Component)]
pub struct CardEnter {
    pub timer: Timer,
}

/// Fly-to-discard animation in progress (then despawn).
#[derive(Component)]
pub struct CardFlyOut {
    pub timer: Timer,
}

pub const ENTER_SECS: f32 = 0.32;
pub const FLYOUT_SECS: f32 = 0.40;
```

Replace `rebuild_hand` with:

```rust
/// Reconcile card entities against the event stream: spawn drawn cards (with a
/// draw-in animation), fly out played/discarded cards, and keep slots in sync.
#[allow(clippy::too_many_arguments)]
pub fn reconcile_hand(
    mut commands: Commands,
    ds: Res<DisplayState>,
    font: Res<UiFont>,
    assets: Res<CardAssets>,
    row: Query<Entity, With<HandRow>>,
    mut flow: MessageReader<crate::anim::CardFlow>,
    mut cards: Query<(Entity, &mut Card), Without<CardFlyOut>>,
) {
    let Ok(row) = row.single() else { return };
    for f in flow.read() {
        match *f {
            crate::anim::CardFlow::Drawn(card) => {
                let slot = cards.iter().count();
                let e = spawn_card(&mut commands, &font, &assets, card, slot, ds.energy);
                commands.entity(e).insert(CardEnter { timer: Timer::from_seconds(ENTER_SECS, TimerMode::Once) });
                commands.entity(row).add_child(e);
            }
            crate::anim::CardFlow::Played { slot } => {
                for (e, mut card) in &mut cards {
                    if card.slot == slot {
                        commands.entity(e).remove::<Button>().insert(CardFlyOut {
                            timer: Timer::from_seconds(FLYOUT_SECS, TimerMode::Once),
                        });
                    } else if card.slot > slot {
                        card.slot -= 1;
                    }
                }
            }
            crate::anim::CardFlow::Discarded => {
                for (e, _) in &mut cards {
                    commands.entity(e).remove::<Button>().insert(CardFlyOut {
                        timer: Timer::from_seconds(FLYOUT_SECS, TimerMode::Once),
                    });
                }
            }
        }
    }
}

/// Darken cards the player can't currently afford (scrim alpha), on energy change.
pub fn refresh_affordability(
    ds: Res<DisplayState>,
    cards: Query<(&Card, &Children)>,
    mut scrims: Query<&mut BackgroundColor, With<CardScrim>>,
) {
    if !ds.is_changed() {
        return;
    }
    for (card, children) in &cards {
        let unaffordable = card.card.spec().cost > ds.energy;
        for child in children.iter() {
            if let Ok(mut bg) = scrims.get_mut(child) {
                bg.0 = bg.0.with_alpha(if unaffordable { 0.5 } else { 0.0 });
            }
        }
    }
}
```

Note: the slot for a newly drawn card is "current live card count" — correct because draw appends in the engine. A card mid-fly-out is excluded from `cards` (via `Without<CardFlyOut>`) so it doesn't take a slot.

- [ ] **Step 2: Swap the systems in the plugin**

In `mod.rs`'s `Update` tuple, replace `hand::rebuild_hand` with `hand::reconcile_hand` and `hand::refresh_affordability` (both in the queue-independent group that already contains the old `rebuild_hand`):

```rust
                    (hand::reconcile_hand, hand::refresh_affordability, sync_texts, highlight_enemies, post_combat)
                        .run_if(in_state(AppState::Combat)),
```

(Keep the input group `(card_click, enemy_click, end_turn_button, keyboard).run_if(...).run_if(queue_empty)` unchanged.)

- [ ] **Step 3: Build**

Run: `cargo build -p helheim`
Expected: compiles. `Children::iter()` in 0.18 yields entities by value or ref — adjust `if let Ok(mut bg) = scrims.get_mut(child)` to `child` vs `*child` per the compiler.

- [ ] **Step 4: Manual checkpoint**

`cargo run -p helheim`, fight: the opening hand still appears; playing a card removes exactly that card and the rest stay put (no full-hand flicker); drawing adds cards; unaffordable cards dim/brighten as energy changes. (Motion is still instant — Tasks 6–9 animate it.)

- [ ] **Step 5: Commit**

```bash
git add crates/helheim/src/screens/combat/
git commit -m "feat(combat): event-sourced hand reconcile + affordability refresh"
```

---

## Task 6: Hover lift

**Files:**
- Modify: `crates/helheim/src/screens/combat/hand.rs`, `mod.rs`

**Interfaces:**
- Consumes: `Card`, `CardEnter`, `CardFlyOut`, `Interaction`, `UiTransform`.
- Produces: `pub fn hover_cards(...)`; pure `fn approach(current: f32, target: f32, dt: f32) -> f32`.

- [ ] **Step 1: Add the eased-approach helper + a test**

In `hand.rs`:

```rust
/// Move `current` a frame-rate-scaled fraction toward `target` (cap at 1.0).
pub fn approach(current: f32, target: f32, rate_dt: f32) -> f32 {
    current + (target - current) * rate_dt.min(1.0)
}

#[cfg(test)]
mod motion_tests {
    use super::approach;
    #[test]
    fn approach_moves_toward_and_clamps() {
        assert!((approach(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        assert!((approach(0.0, 1.0, 5.0) - 1.0).abs() < 1e-6); // clamped
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p helheim approach_moves_toward_and_clamps`
Expected: PASS.

- [ ] **Step 3: Add the hover system**

In `hand.rs`:

```rust
const HOVER_SCALE: f32 = 1.09;
const HOVER_LIFT: f32 = -14.0;

/// Hovered settled card eases up + scales; others settle back to identity.
pub fn hover_cards(
    time: Res<Time>,
    mut cards: Query<(&Interaction, &mut UiTransform), (With<Card>, Without<CardEnter>, Without<CardFlyOut>)>,
) {
    let dt = time.delta_secs() * 14.0;
    for (interaction, mut tf) in &mut cards {
        let hot = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let ts = if hot { HOVER_SCALE } else { 1.0 };
        let ty = if hot { HOVER_LIFT } else { 0.0 };
        let s = approach(tf.scale.x, ts, dt);
        tf.scale = Vec2::splat(s);
        let y = approach(px_y(&tf.translation), ty, dt);
        tf.translation = Val2::px(0.0, y);
    }
}

/// Read the px component of a `Val2`'s y (0 if not a `Px`).
fn px_y(t: &Val2) -> f32 {
    if let Val::Px(p) = t.y { p } else { 0.0 }
}
```

- [ ] **Step 4: Register + build + checkpoint**

Add `hand::hover_cards` to the `run_if(in_state(AppState::Combat))` group in `mod.rs`. Run `cargo build -p helheim` (fix `Val2`/`Val::Px` access per compiler), then `cargo run -p helheim` and confirm hovering a card lifts/scales it and it settles back on exit.

- [ ] **Step 5: Commit**

```bash
git add crates/helheim/src/screens/combat/
git commit -m "feat(combat): hover-lift cards via UiTransform"
```

---

## Task 7: Draw-in animation

**Files:**
- Modify: `crates/helheim/src/screens/combat/hand.rs`, `mod.rs`

**Interfaces:**
- Consumes: `CardEnter`, `UiTransform`, `approach`.
- Produces: `pub fn animate_enter(...)`.

- [ ] **Step 1: Add the system**

In `hand.rs`:

```rust
/// Slide a freshly drawn card in from the draw pile (lower-left) to rest.
pub fn animate_enter(
    time: Res<Time>,
    mut commands: Commands,
    mut cards: Query<(Entity, &mut CardEnter, &mut UiTransform)>,
) {
    for (e, mut enter, mut tf) in &mut cards {
        enter.timer.tick(time.delta());
        let t = enter.timer.fraction();
        let ease = t * t * (3.0 - 2.0 * t); // smoothstep
        tf.translation = Val2::px(-280.0 * (1.0 - ease), 120.0 * (1.0 - ease));
        tf.scale = Vec2::splat(0.5 + 0.5 * ease);
        if enter.timer.is_finished() {
            tf.translation = Val2::px(0.0, 0.0);
            tf.scale = Vec2::ONE;
            commands.entity(e).remove::<CardEnter>();
        }
    }
}
```

- [ ] **Step 2: Register + build + checkpoint**

Add `hand::animate_enter` to the combat `Update` group. `cargo build -p helheim`, then `cargo run -p helheim`: at the start of a fight and on draws, cards slide/scale in from the lower-left and settle. Multiple draws stagger naturally because their timers start as each `CardFlow::Drawn` is processed.

- [ ] **Step 3: Commit**

```bash
git add crates/helheim/src/screens/combat/
git commit -m "feat(combat): draw-in card animation"
```

---

## Task 8: Play → fly-to-discard

**Files:**
- Modify: `crates/helheim/src/screens/combat/hand.rs`, `mod.rs`

**Interfaces:**
- Consumes: `CardFlyOut`, `UiTransform`.
- Produces: `pub fn animate_flyout(...)`.

- [ ] **Step 1: Add the system**

In `hand.rs`:

```rust
/// Fly a played/discarded card to the discard pile (lower-right), then despawn.
pub fn animate_flyout(
    time: Res<Time>,
    mut commands: Commands,
    mut cards: Query<(Entity, &mut CardFlyOut, &mut Node, &mut UiTransform)>,
) {
    for (e, mut out, mut node, mut tf) in &mut cards {
        node.position_type = PositionType::Absolute; // pop out of the row so the rest reflow
        out.timer.tick(time.delta());
        let t = out.timer.fraction();
        tf.translation = Val2::px(320.0 * t, 130.0 * t);
        tf.scale = Vec2::splat(1.0 - 0.65 * t);
        if out.timer.is_finished() {
            commands.entity(e).despawn();
        }
    }
}
```

- [ ] **Step 2: Register + build + checkpoint**

Add `hand::animate_flyout` to the combat `Update` group. `cargo build -p helheim`, then `cargo run -p helheim`: playing a card sends it arcing to the lower-right while shrinking, then it disappears; the remaining cards reflow to fill the gap.

- [ ] **Step 3: Commit**

```bash
git add crates/helheim/src/screens/combat/
git commit -m "feat(combat): play → fly-to-discard animation"
```

---

## Task 9: Targeting pulse

**Files:**
- Modify: `crates/helheim/src/screens/combat/hand.rs`, `mod.rs`

**Interfaces:**
- Consumes: `PendingCard` (from `mod.rs`), `Card`, `CardFlyOut`, `UiTransform`, `BorderColor`, `kind_color`.
- Produces: `pub fn pulse_pending(...)`. `PendingCard` must be readable from `hand.rs` — make it `pub(crate)`.

- [ ] **Step 1: Expose `PendingCard`**

In `mod.rs`, change `#[derive(Resource, Default)] struct PendingCard(Option<usize>);` to `#[derive(Resource, Default)] pub(crate) struct PendingCard(pub(crate) Option<usize>);`.

- [ ] **Step 2: Add the pulse system**

In `hand.rs`:

```rust
/// The card awaiting an enemy target pulses (scale + brightened border).
pub fn pulse_pending(
    time: Res<Time>,
    pending: Res<super::PendingCard>,
    mut cards: Query<(&Card, &mut UiTransform, &mut BorderColor), Without<CardFlyOut>>,
) {
    let wave = (time.elapsed_secs() * 6.0).sin() * 0.5 + 0.5; // 0..1
    for (card, mut tf, mut border) in &mut cards {
        let base = kind_color(card.card.spec().kind);
        if pending.0 == Some(card.slot) {
            tf.scale = Vec2::splat(1.0 + 0.06 * wave);
            border.0 = base.mix(&Color::WHITE, 0.3 + 0.4 * wave);
        } else if border.0 != base {
            border.0 = base; // restore once no longer pending
        }
    }
}
```

(`BorderColor`'s field is `.0`; `Color::mix` blends. If `BorderColor` is a struct with named fields in 0.18, set the all-sides color per the compiler.)

- [ ] **Step 3: Order it after hover, register, build, checkpoint**

In `mod.rs`, add `hand::pulse_pending` to the combat `Update` group **after** `hover_cards` (so the pending card's scale isn't overwritten by hover the same frame) — express with `.after(hand::hover_cards)` if needed. Also have `hover_cards` skip the pending card: add a guard `if pending.0 == Some(card.slot) { continue; }` — to do that, give `hover_cards` a `pending: Res<super::PendingCard>` param and the matching `Card` reference (query `(&Interaction, &Card, &mut UiTransform)`).

Run `cargo build -p helheim`, then `cargo run -p helheim`: play a multi-target-needed card (`Targeting::SingleEnemy` with >1 living enemy) so targeting arms — the chosen card should pulse with a glowing border until you click an enemy or press Esc.

- [ ] **Step 4: Commit**

```bash
git add crates/helheim/src/screens/combat/
git commit -m "feat(combat): targeting-pulse on the pending card"
```

---

## Task 10: Integration — lint, tests, play-test, memory

**Files:**
- Modify: `README.md` (only if controls changed — they didn't; cards still play by click / 1–9/0)
- Modify: project memory

- [ ] **Step 1: Full workspace tests**

Run: `cargo test --workspace`
Expected: all pass (anim `card_flow` test, `kind_color`, `approach`, `card_assets_load_at_plugin_build`, plus all existing core/shell tests). Report the summary lines.

- [ ] **Step 2: Strict clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean (only the pre-existing third-party `block v0.1.6` note). Fix any warnings in the new combat code (e.g. add `#[allow(clippy::too_many_arguments)]` to systems exceeding 7 params — `reconcile_hand`, possibly `pulse_pending`/`hover_cards` if they grew). Re-run until exit 0.

- [ ] **Step 3: Full manual play-test checklist**

Run `cargo run -p helheim -- --seed 7`, climb into fights, and confirm:
- [ ] Cards show per-type color + icon (sword/shield/sparkle) + frame + cost gem.
- [ ] Unaffordable cards are dimmed; they brighten when energy allows.
- [ ] Hovering a card lifts it; it settles on exit.
- [ ] Playing a card flies it to the discard (lower-right); the rest reflow.
- [ ] Drawing slides cards in from the draw pile (lower-left); the opening hand deals in.
- [ ] A card needing a target pulses until you pick an enemy (and stops on Esc/confirm).
- [ ] Keyboard (1–9/0, Tab/arrows, Enter, E) and mouse both work; combat still resolves and routes (reward/defeat/victory/map).

- [ ] **Step 4: Update project memory**

Note in the `phase-2-progress` memory that combat cards now have type identity + the four animations (and the `CardFlow` message / `screens/combat/` module split), so future sessions don't re-flag flat cards.

---

## Self-Review

**Spec coverage:**
- `CardFlow` message + pure mapping + emission → Task 1. ✓
- Card-type icons (sword/shield/sparkle) → Task 2. ✓
- `screens/combat/` module + `kind_color` + `CardAssets` → Task 3. ✓
- Option-C visuals (type frame, icon, watermark, cost gem, affordability) → Task 4. ✓
- Event-sourced reconcile + persistent entities + slot tracking + affordability refresh → Task 5. ✓
- Four motions: hover (Task 6), draw-in (Task 7), fly-out (Task 8), targeting pulse (Task 9). ✓
- Tests: `kind_color`, `card_flow` mapping, `approach`/offset math, headless `CardAssets` load → Tasks 1, 3, 6. ✓
- `helheim_core` untouched → no task modifies it. ✓
- Lint/test/play-test/memory → Task 10. ✓

**Placeholder scan:** No TBD/"handle errors"/"similar to Task N". Volatile-API notes give concrete fallbacks. Manual play-test steps are explicit observations, not vague.

**Type consistency:** `CardFlow::{Drawn(CardId), Played{slot}, Discarded}` defined in Task 1, consumed identically in Task 5. `Card { slot, card }`, `CardEnter`, `CardFlyOut`, `CardScrim` defined in Tasks 4–5 and used with the same fields in Tasks 5–9. `kind_color`, `CardAssets::for_kind`, `spawn_card`, `approach` signatures match across tasks. The slot semantics (draw appends at tail; play decrements higher slots) are consistent between the reconcile and the engine's hand model.

**Decisions flagged for the implementer:** affordability dimming uses a single full-card scrim node (uniform, one component to toggle) rather than re-tinting each child; reflow on play is immediate (the flying card pops to absolute) — smoothing it is an explicit non-goal; gizmo-free (all UI). Several `BorderColor`/`Val2`/`Children::iter` access forms are flagged as compiler-adjust spots.
