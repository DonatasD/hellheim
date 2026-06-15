# Map Visual Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the Act 1 map screen as an atmospheric world-space Bevy scene with per-kind node icons, curved trail paths, a traveling player token, glow/pulse/hover/entrance animations, a following camera, fog over unreached floors, and mouse + keyboard navigation.

**Architecture:** World-space 2D scene under the existing global `Camera2d`, with a thin Bevy UI overlay for the HUD. All map math (positions, bézier curves, camera clamp, node picking, keyboard cycling, per-kind colors, node/edge visual classification) lives in a **pure, unit-tested** `layout.rs`; the Bevy systems are thin wiring that call those helpers. `helheim_core` is untouched — the map graph already exposes node kinds, `next` edges, `available_nodes()`, and `position`.

**Tech Stack:** Rust, Bevy 0.18 (`Mesh2d` + `ColorMaterial` for node discs/rings/glow/fog, `Sprite` + `Image` for tinted icons, `Gizmos` for edge curves, `Transform` lerps for animation, `ButtonInput` for keyboard, cursor→world for mouse), `helheim_core` (read-only).

**Spec:** [`docs/superpowers/specs/2026-06-15-map-visual-overhaul-design.md`](../specs/2026-06-15-map-visual-overhaul-design.md)

---

## File Structure

Promote the single file [`crates/helheim/src/screens/map.rs`](../../../crates/helheim/src/screens/map.rs) (188 lines) into a `screens/map/` module. `screens/mod.rs` already declares `pub mod map;`, which resolves to `map/mod.rs` unchanged.

| File | Responsibility |
|---|---|
| `crates/helheim/src/screens/map/mod.rs` | `MapPlugin`, shared components/resources, `MapAssets` (icon handles loaded at build time), system wiring, lifecycle |
| `crates/helheim/src/screens/map/layout.rs` | **Pure, unit-tested** geometry + visual classification — no Bevy ECS, no queries |
| `crates/helheim/src/screens/map/scene.rs` | Spawn the scene on enter (camera framing, nodes, glow, fog, HUD), despawn on exit, draw edges (gizmos) |
| `crates/helheim/src/screens/map/motion.rs` | The four animations + camera-follow |
| `crates/helheim/src/screens/map/input.rs` | Mouse picking + keyboard selection → travel; token-travel animation gating `enter_node` + routing |
| `crates/helheim/assets/icons/*.png` | Five tinted-at-runtime silhouette icons |
| `CREDITS.md` | CC BY attribution for the icon set |

**Design note (deviations from spec, for the better, both keep spec intent):**
- The spec listed pure geometry in `layout.rs`; the pure **visual classifiers** (`kind_color`, `node_visual`, `edge_style`) also go in `layout.rs` since they are pure and benefit from the same unit tests. This follows the existing [`anim.rs`](../../../crates/helheim/src/anim.rs) precedent (pure functions + tests beside Bevy systems).
- The spec mentioned a `glow.png` and a fog "gradient overlay." To avoid sourcing/maintaining extra image assets, **glow and fog are rendered with translucent `Mesh2d` circles/rectangles** instead. The only external image assets are the five icon PNGs.

---

## Task 1: Pure layout geometry — positions & curves

**Files:**
- Create: `crates/helheim/src/screens/map/layout.rs`

- [ ] **Step 1: Create `layout.rs` with constants and the failing test for positions/curves**

```rust
//! Pure map geometry & visual classification — no Bevy ECS, fully unit-tested.
use bevy::prelude::*;
use helheim_core::map::{NodeId, NodeKind, BOSS_FLOOR, MAP_WIDTH};

use crate::theme;

/// Vertical world units between floors.
pub const FLOOR_GAP: f32 = 120.0;
/// Horizontal world units between columns.
pub const COL_GAP: f32 = 110.0;
/// Node hit/visual radius in world units.
pub const NODE_R: f32 = 30.0;
/// Camera-follow clamp: enough offset to frame floor 1 without overscrolling.
pub const MIN_CAM_Y: f32 = 2.0 * FLOOR_GAP;
pub const MAX_CAM_Y: f32 = (BOSS_FLOOR as f32 - 1.0) * FLOOR_GAP;

/// World position of a node: column centered horizontally, floor stacked upward.
pub fn node_pos(id: NodeId) -> Vec2 {
    let x = (id.col as f32 - (MAP_WIDTH as f32 - 1.0) / 2.0) * COL_GAP;
    let y = id.floor as f32 * FLOOR_GAP;
    Vec2::new(x, y)
}

fn cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

/// Point at fraction `t` along the vertical-tangent cubic between two nodes.
pub fn bezier_point_at(from: Vec2, to: Vec2, t: f32) -> Vec2 {
    let my = (from.y + to.y) * 0.5;
    cubic(from, Vec2::new(from.x, my), Vec2::new(to.x, my), to, t)
}

/// `n + 1` samples along the curve (endpoints included).
pub fn bezier_points(from: Vec2, to: Vec2, n: usize) -> Vec<Vec2> {
    (0..=n).map(|i| bezier_point_at(from, to, i as f32 / n as f32)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_pos_climbs_and_centers() {
        assert!(node_pos(NodeId { floor: 2, col: 0 }).y > node_pos(NodeId { floor: 1, col: 0 }).y);
        assert!(node_pos(NodeId { floor: 1, col: 6 }).x > node_pos(NodeId { floor: 1, col: 0 }).x);
        // MAP_WIDTH == 7, so the middle column (3) sits on the x axis.
        assert!(node_pos(NodeId { floor: 1, col: 3 }).x.abs() < 1e-3);
    }

    #[test]
    fn bezier_hits_its_endpoints() {
        let a = Vec2::new(0., 0.);
        let b = Vec2::new(110., 120.);
        let pts = bezier_points(a, b, 8);
        assert_eq!(pts.len(), 9);
        assert!((pts[0] - a).length() < 1e-3);
        assert!((pts[8] - b).length() < 1e-3);
    }
}
```

- [ ] **Step 2: Declare the module so the test compiles**

In `crates/helheim/src/screens/map.rs` — this file is replaced wholesale in Task 4, but to compile `layout.rs` now, temporarily add at the top of the existing `map.rs`:

```rust
mod layout;
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p helheim layout::tests -- --nocapture`
Expected: PASS (2 tests: `node_pos_climbs_and_centers`, `bezier_hits_its_endpoints`).

- [ ] **Step 4: Commit**

```bash
git add crates/helheim/src/screens/map/layout.rs crates/helheim/src/screens/map.rs
git commit -m "feat(map): pure node positions and bézier curve sampling"
```

---

## Task 2: Pure layout interaction — camera clamp, picking, keyboard cycle

**Files:**
- Modify: `crates/helheim/src/screens/map/layout.rs`

- [ ] **Step 1: Add the functions and their failing tests**

Append the three functions inside `layout.rs` (above the `#[cfg(test)]` module):

```rust
/// Camera-follow target for a floor, clamped so the view never overscrolls.
pub fn camera_y_for(floor: u8) -> f32 {
    (floor as f32 * FLOOR_GAP).clamp(MIN_CAM_Y, MAX_CAM_Y)
}

/// Nearest node whose center is within `NODE_R` of a world-space cursor point.
pub fn pick_node(cursor: Vec2, nodes: &[(NodeId, Vec2)]) -> Option<NodeId> {
    nodes
        .iter()
        .filter(|(_, p)| p.distance(cursor) <= NODE_R)
        .min_by(|a, b| {
            a.1.distance(cursor)
                .partial_cmp(&b.1.distance(cursor))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, _)| *id)
}

/// Step the keyboard selection across the reachable set (sorted by column),
/// wrapping at both ends. `dir` is +1 (right) or -1 (left).
pub fn keyboard_step(current: NodeId, reachable: &[NodeId], dir: i8) -> NodeId {
    let mut sorted = reachable.to_vec();
    sorted.sort_by_key(|n| (n.floor, n.col));
    if sorted.is_empty() {
        return current;
    }
    let idx = sorted.iter().position(|&n| n == current).unwrap_or(0) as i32;
    let len = sorted.len() as i32;
    sorted[(idx + dir as i32).rem_euclid(len) as usize]
}
```

Add these tests inside the existing `mod tests`:

```rust
    #[test]
    fn camera_clamps_at_both_ends_and_climbs_between() {
        assert_eq!(camera_y_for(1), MIN_CAM_Y); // below the frame floor → clamped up
        assert_eq!(camera_y_for(BOSS_FLOOR), MAX_CAM_Y);
        assert!(camera_y_for(8) > camera_y_for(4));
    }

    #[test]
    fn pick_node_returns_nearest_within_radius() {
        let nodes = vec![
            (NodeId { floor: 1, col: 0 }, Vec2::new(0., 0.)),
            (NodeId { floor: 1, col: 1 }, Vec2::new(200., 0.)),
        ];
        assert_eq!(pick_node(Vec2::new(5., 5.), &nodes), Some(NodeId { floor: 1, col: 0 }));
        assert_eq!(pick_node(Vec2::new(500., 500.), &nodes), None);
    }

    #[test]
    fn keyboard_step_cycles_and_wraps() {
        let r = vec![
            NodeId { floor: 2, col: 1 },
            NodeId { floor: 2, col: 3 },
            NodeId { floor: 2, col: 5 },
        ];
        assert_eq!(keyboard_step(r[0], &r, 1), r[1]);
        assert_eq!(keyboard_step(r[2], &r, 1), r[0]); // wrap forward
        assert_eq!(keyboard_step(r[0], &r, -1), r[2]); // wrap backward
    }
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cargo test -p helheim layout::tests`
Expected: PASS (5 tests total now).

- [ ] **Step 3: Commit**

```bash
git add crates/helheim/src/screens/map/layout.rs
git commit -m "feat(map): pure camera clamp, node picking, keyboard cycling"
```

---

## Task 3: Pure visual classification — colors, node & edge states

**Files:**
- Modify: `crates/helheim/src/screens/map/layout.rs`

- [ ] **Step 1: Add the enums, functions, and failing tests**

Append inside `layout.rs` (above `#[cfg(test)]`):

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeVisual {
    Current,
    Reachable,
    Visited,
    Locked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeStyle {
    Taken,     // road through already-climbed floors
    Available, // leaves the current node to a reachable child
    Ahead,     // unreached future road
}

/// Per-kind accent color for rings, icon tint, and glow.
pub fn kind_color(kind: NodeKind) -> Color {
    match kind {
        NodeKind::Monster => Color::srgb(0.55, 0.56, 0.60), // steel
        NodeKind::Elite => theme::ACCENT,                   // red
        NodeKind::Rest => Color::srgb(0.95, 0.64, 0.23),    // warm orange
        NodeKind::Treasure => theme::ENERGY_COLOR,          // gold
        NodeKind::Boss => Color::srgb(0.89, 0.70, 0.29),    // bright gold
    }
}

/// Classify a node for rendering, given where the player stands and what's reachable.
pub fn node_visual(id: NodeId, current: Option<NodeId>, reachable: &[NodeId]) -> NodeVisual {
    if Some(id) == current {
        NodeVisual::Current
    } else if reachable.contains(&id) {
        NodeVisual::Reachable
    } else if id.floor < current.map(|c| c.floor).unwrap_or(0) {
        NodeVisual::Visited
    } else {
        NodeVisual::Locked
    }
}

/// Classify an edge `from → to` for rendering.
pub fn edge_style(
    from: NodeId,
    to: NodeId,
    current: Option<NodeId>,
    reachable: &[NodeId],
) -> EdgeStyle {
    if Some(from) == current && reachable.contains(&to) {
        EdgeStyle::Available
    } else if to.floor <= current.map(|c| c.floor).unwrap_or(0) {
        EdgeStyle::Taken
    } else {
        EdgeStyle::Ahead
    }
}
```

Add these tests inside `mod tests` (NodeKind is already imported via `super::*`):

```rust
    #[test]
    fn kind_color_is_distinct_and_themed() {
        assert_eq!(kind_color(NodeKind::Elite), theme::ACCENT);
        assert_ne!(kind_color(NodeKind::Monster), kind_color(NodeKind::Rest));
    }

    #[test]
    fn node_visual_classifies_by_position_and_reach() {
        let cur = NodeId { floor: 3, col: 2 };
        let reach = vec![NodeId { floor: 4, col: 1 }, NodeId { floor: 4, col: 2 }];
        assert_eq!(node_visual(cur, Some(cur), &reach), NodeVisual::Current);
        assert_eq!(node_visual(reach[0], Some(cur), &reach), NodeVisual::Reachable);
        assert_eq!(node_visual(NodeId { floor: 1, col: 0 }, Some(cur), &reach), NodeVisual::Visited);
        assert_eq!(node_visual(NodeId { floor: 6, col: 0 }, Some(cur), &reach), NodeVisual::Locked);
    }

    #[test]
    fn edge_style_classifies_taken_available_ahead() {
        let cur = NodeId { floor: 3, col: 2 };
        let reach = vec![NodeId { floor: 4, col: 2 }];
        assert_eq!(edge_style(cur, reach[0], Some(cur), &reach), EdgeStyle::Available);
        assert_eq!(
            edge_style(NodeId { floor: 1, col: 0 }, NodeId { floor: 2, col: 0 }, Some(cur), &reach),
            EdgeStyle::Taken
        );
        assert_eq!(
            edge_style(NodeId { floor: 5, col: 0 }, NodeId { floor: 6, col: 0 }, Some(cur), &reach),
            EdgeStyle::Ahead
        );
    }
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cargo test -p helheim layout::tests`
Expected: PASS (8 tests total).

- [ ] **Step 3: Commit**

```bash
git add crates/helheim/src/screens/map/layout.rs
git commit -m "feat(map): pure per-kind colors and node/edge visual classification"
```

---

## Task 4: Module scaffold — `MapPlugin`, types, assets, headless smoke test

This replaces the old `map.rs` with `map/mod.rs`: shared types/resources/components, build-time icon loading (mirroring how the font loads in [`theme.rs:27-30`](../../../crates/helheim/src/theme.rs#L27-L30)), and an empty system wiring we fill in later tasks.

**Files:**
- Create: `crates/helheim/src/screens/map/mod.rs`
- Delete: `crates/helheim/src/screens/map.rs`
- Test: headless test inside `mod.rs`

- [ ] **Step 1: Delete the old single-file screen**

```bash
git rm crates/helheim/src/screens/map.rs
```

- [ ] **Step 2: Create `map/mod.rs` with types, asset loading, and the plugin skeleton**

```rust
//! World-space Act 1 map screen. Pure math lives in `layout`; the systems here
//! and in the sibling modules are thin Bevy wiring over it.
use bevy::prelude::*;
use helheim_core::map::{NodeId, NodeKind};

mod input;
mod layout;
mod motion;
mod scene;

use crate::AppState;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        // Icons must exist before `OnEnter(Map)` runs, so load them at build time
        // (the AssetServer is already present — DefaultPlugins built it), exactly
        // like the font in ThemePlugin.
        let assets = MapAssets::load(app.world().resource::<AssetServer>());
        app.insert_resource(assets)
            .add_systems(OnEnter(AppState::Map), scene::spawn_map)
            .add_systems(OnExit(AppState::Map), scene::despawn_map)
            .add_systems(
                Update,
                (
                    scene::draw_edges,
                    motion::pulse_auras,
                    motion::reveal_nodes,
                    motion::hover_lift,
                    motion::follow_camera,
                    input::navigate,
                    motion::travel_token,
                )
                    .run_if(in_state(AppState::Map)),
            );
    }
}

/// Icon textures, loaded once and tinted per node kind at spawn time.
#[derive(Resource)]
pub struct MapAssets {
    pub fight: Handle<Image>,
    pub elite: Handle<Image>,
    pub rest: Handle<Image>,
    pub treasure: Handle<Image>,
    pub boss: Handle<Image>,
}

impl MapAssets {
    fn load(server: &AssetServer) -> Self {
        MapAssets {
            fight: server.load("icons/fight.png"),
            elite: server.load("icons/elite.png"),
            rest: server.load("icons/rest.png"),
            treasure: server.load("icons/treasure.png"),
            boss: server.load("icons/boss.png"),
        }
    }

    pub fn for_kind(&self, kind: NodeKind) -> Handle<Image> {
        match kind {
            NodeKind::Monster => self.fight.clone(),
            NodeKind::Elite => self.elite.clone(),
            NodeKind::Rest => self.rest.clone(),
            NodeKind::Treasure => self.treasure.clone(),
            NodeKind::Boss => self.boss.clone(),
        }
    }
}

/// Despawn root for everything spawned by the map screen.
#[derive(Component)]
pub struct MapScene;

/// Tags a node entity (the body; ring + icon are its children).
#[derive(Component)]
pub struct MapNodeEnt(pub NodeId);

/// The player marker that walks the road.
#[derive(Component)]
pub struct PlayerToken;

/// A pulsing glow disc behind a reachable node.
#[derive(Component)]
pub struct GlowAura;

/// One-shot entrance scale-in; removed when finished.
#[derive(Component)]
pub struct Reveal {
    pub timer: Timer,
}

/// Keyboard-highlighted reachable node.
#[derive(Resource)]
pub struct MapSelection(pub NodeId);

/// Present only while the token is walking to a chosen node; gates the state
/// transition until the animation finishes.
#[derive(Resource)]
pub struct Traveling {
    pub to: NodeId,
    pub from: Vec2,
    pub timer: Timer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemePlugin;
    use crate::{AppState, CliSeed, Session};
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use helheim_core::run::RunState;

    /// Regression mirroring the menu test: `MapAssets` must exist after the
    /// plugin builds, so `OnEnter(Map)` never hits a missing resource.
    #[test]
    fn map_assets_load_at_plugin_build() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));
        app.init_asset::<bevy::text::Font>();
        app.init_asset::<Image>();
        app.init_state::<AppState>();
        app.insert_resource(CliSeed(Some(0)));
        app.insert_resource(Session { run: RunState::new(0) });
        app.add_plugins((ThemePlugin, MapPlugin));
        app.update();
        assert!(app.world().contains_resource::<MapAssets>());
    }
}
```

- [ ] **Step 3: Create empty sibling stubs so `mod.rs` compiles**

The `mod input; mod motion; mod scene;` declarations reference systems we build next. Create minimal stubs now so the crate compiles and the smoke test runs.

Create `crates/helheim/src/screens/map/scene.rs`:

```rust
use bevy::prelude::*;

use super::MapScene;
use crate::Session;

pub fn spawn_map(_commands: Commands, _session: Res<Session>) {}
pub fn despawn_map(mut commands: Commands, q: Query<Entity, With<MapScene>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
pub fn draw_edges() {}
```

Create `crates/helheim/src/screens/map/motion.rs`:

```rust
pub fn pulse_auras() {}
pub fn reveal_nodes() {}
pub fn hover_lift() {}
pub fn follow_camera() {}
pub fn travel_token() {}
```

Create `crates/helheim/src/screens/map/input.rs`:

```rust
pub fn navigate() {}
```

- [ ] **Step 4: Verify `RunState::new` signature**

Run: `grep -n "pub fn new" crates/helheim_core/src/run.rs`
Expected: a constructor taking a seed, e.g. `pub fn new(seed: u64) -> Self`. If the name/signature differs, adjust the `Session { run: RunState::new(0) }` line in the test and in `main.rs`'s existing usage to match (search `RunState::` in `crates/helheim/src/`).

- [ ] **Step 5: Build and run the smoke test**

Run: `cargo test -p helheim map::tests::map_assets_load_at_plugin_build`
Expected: PASS. (If it fails to compile on `RunState::new`, fix per Step 4.)

- [ ] **Step 6: Confirm the whole crate still builds**

Run: `cargo build -p helheim`
Expected: builds (empty-system stubs produce no warnings; if Bevy warns about unused empty systems it will not — they are registered).

- [ ] **Step 7: Commit**

```bash
git add crates/helheim/src/screens/map/
git commit -m "feat(map): module scaffold — MapPlugin, types, build-time icon load"
```

---

## Task 5: Source the icon assets + attribution

The renderer tints white silhouettes per kind. Get five white-on-transparent PNGs (≈128×128) from **game-icons.net** (CC BY 3.0). The exact icon is flexible as long as it reads as the right symbol.

**Files:**
- Create: `crates/helheim/assets/icons/fight.png`, `elite.png`, `rest.png`, `treasure.png`, `boss.png`
- Create: `CREDITS.md`
- Modify: `README.md`

- [ ] **Step 1: Download five icons as white PNGs into `crates/helheim/assets/icons/`**

Suggested icons from game-icons.net (download with **white** foreground, transparent background):
- `fight.png` — "crossed-swords" (by Lorc)
- `elite.png` — "skull-crossed-bones" or "spiked-skull" (by Lorc)
- `rest.png` — "campfire" (by Delapouite)
- `treasure.png` — "open-treasure-chest" / "locked-chest" (by Delapouite)
- `boss.png` — "crown" / "skull-crown" (by Lorc)

If offline, create temporary solid white placeholder discs so development can proceed (swap real icons in later — no code change):

```bash
# Requires ImageMagick. Placeholder = white filled circle on transparent bg.
mkdir -p crates/helheim/assets/icons
for n in fight elite rest treasure boss; do
  magick -size 128x128 xc:none -fill white -draw "circle 64,64 64,16" \
    crates/helheim/assets/icons/$n.png
done
```

- [ ] **Step 2: Create `CREDITS.md` with the CC BY attribution**

```markdown
# Credits

## Icons

Map node icons are from [game-icons.net](https://game-icons.net), used under the
[Creative Commons Attribution 3.0 Unported (CC BY 3.0)](https://creativecommons.org/licenses/by/3.0/)
license.

- Crossed Swords — Lorc
- Skull — Lorc
- Campfire — Delapouite
- Treasure Chest — Delapouite
- Crown — Lorc

(Update author/icon names above to match the exact icons you downloaded.)

## Font

Fira Sans — SIL Open Font License.
```

- [ ] **Step 3: Add a one-line pointer from the README**

In `README.md`, under the existing `Font:` line near the bottom, add:

```markdown
Icons: game-icons.net (CC BY 3.0) — see [CREDITS.md](CREDITS.md).
```

- [ ] **Step 4: Verify the files are present**

Run: `ls -1 crates/helheim/assets/icons/`
Expected: `boss.png  elite.png  fight.png  rest.png  treasure.png`

- [ ] **Step 5: Commit**

```bash
git add crates/helheim/assets/icons/ CREDITS.md README.md
git commit -m "assets(map): add CC BY node icons + attribution"
```

---

## Task 6: Spawn the scene — camera framing, nodes, glow, fog, HUD

Fill in `scene::spawn_map`. Each node is a parent **body** disc (`Mesh2d` circle) with two children — a **ring** disc behind it (colored, slightly larger) and the tinted **icon** sprite in front. A translucent **glow** disc is a separate sibling behind reachable nodes. Fog is translucent rectangles near the top. The HUD is a UI overlay.

**Files:**
- Modify: `crates/helheim/src/screens/map/scene.rs`
- Reference: [`theme.rs`](../../../crates/helheim/src/theme.rs) (colors, `text`, `UiFont`), [`map.rs` git history] for the old HUD strings

- [ ] **Step 1: Replace `scene.rs` `spawn_map` with the full scene builder**

```rust
use bevy::prelude::*;
use helheim_core::map::{NodeId, BOSS_FLOOR};

use super::layout::{self, NodeVisual};
use super::{GlowAura, MapAssets, MapNodeEnt, MapScene, MapSelection, Reveal};
use crate::theme::{self, UiFont};
use crate::{AppState, Session};

const Z_GLOW: f32 = 1.0;
const Z_RING: f32 = 1.8;
const Z_BODY: f32 = 2.0;
const Z_ICON: f32 = 3.0;
const Z_FOG: f32 = 5.0;

/// Dim factor applied to locked/visited nodes.
fn dim(c: Color, factor: f32) -> Color {
    let l = c.to_linear();
    Color::linear_rgba(l.red * factor, l.green * factor, l.blue * factor, l.alpha)
}

pub fn spawn_map(
    mut commands: Commands,
    session: Res<Session>,
    assets: Res<MapAssets>,
    font: Res<UiFont>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let run = &session.run;
    let reachable = run.available_nodes();
    let current = run.position;
    let cur_floor = current.map(|c| c.floor).unwrap_or(0);

    // Frame the camera on the current floor.
    if let Ok(mut cam) = cameras.single_mut() {
        cam.translation.x = 0.0;
        cam.translation.y = layout::camera_y_for(cur_floor);
    }

    // Seed keyboard selection to the lowest-column reachable node.
    if let Some(first) = reachable.iter().min_by_key(|n| (n.floor, n.col)) {
        commands.insert_resource(MapSelection(*first));
    }

    let circle = meshes.add(Circle::new(layout::NODE_R));

    // Root for despawn.
    let root = commands.spawn((MapScene, Transform::default(), Visibility::Visible)).id();

    // Nodes.
    for node in run.map.all() {
        let id = node.id;
        let pos = layout::node_pos(id);
        let vis = layout::node_visual(id, current, &reachable);
        let base = layout::kind_color(node.kind);

        let (ring_color, icon_color, locked) = match vis {
            NodeVisual::Current => (theme::ACCENT, Color::WHITE, false),
            NodeVisual::Reachable => (base, Color::WHITE, false),
            NodeVisual::Visited => (dim(base, 0.35), dim(Color::WHITE, 0.4), true),
            NodeVisual::Locked => (dim(base, 0.25), dim(Color::WHITE, 0.3), true),
        };

        // Body (parent) — dark disc.
        let body = commands
            .spawn((
                MapNodeEnt(id),
                Reveal { timer: Timer::from_seconds(0.45, TimerMode::Once) },
                Mesh2d(circle.clone()),
                MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::srgb(0.12, 0.12, 0.16)))),
                Transform::from_xyz(pos.x, pos.y, Z_BODY).with_scale(Vec3::ZERO),
            ))
            .id();

        // Ring (child, behind) — colored, larger disc.
        let ring = commands
            .spawn((
                Mesh2d(meshes.add(Circle::new(layout::NODE_R + 4.0))),
                MeshMaterial2d(materials.add(ColorMaterial::from_color(ring_color))),
                Transform::from_xyz(0.0, 0.0, Z_RING - Z_BODY),
            ))
            .id();

        // Icon (child, front) — tinted sprite.
        let icon = commands
            .spawn((
                Sprite {
                    image: assets.for_kind(node.kind),
                    color: icon_color,
                    custom_size: Some(Vec2::splat(layout::NODE_R * 1.4)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, Z_ICON - Z_BODY),
            ))
            .id();

        commands.entity(body).add_children(&[ring, icon]);
        commands.entity(root).add_child(body);

        // Glow behind reachable nodes (separate sibling so it pulses independently).
        if matches!(vis, NodeVisual::Reachable | NodeVisual::Current) {
            let glow = commands
                .spawn((
                    GlowAura,
                    Mesh2d(meshes.add(Circle::new(layout::NODE_R * 1.7))),
                    MeshMaterial2d(materials.add(ColorMaterial::from_color(base.with_alpha(0.22)))),
                    Transform::from_xyz(pos.x, pos.y, Z_GLOW),
                ))
                .id();
            commands.entity(root).add_child(glow);
        }

        let _ = locked; // (locked already folded into colors above)
    }

    // Fog: three stacked translucent dark quads above the boss, fading downward.
    let fog_w = layout::COL_GAP * 8.0;
    let top_y = BOSS_FLOOR as f32 * layout::FLOOR_GAP;
    for (i, alpha) in [0.85_f32, 0.55, 0.25].into_iter().enumerate() {
        let band = commands
            .spawn((
                Mesh2d(meshes.add(Rectangle::new(fog_w, layout::FLOOR_GAP))),
                MeshMaterial2d(materials.add(ColorMaterial::from_color(
                    Color::srgb(0.05, 0.05, 0.08).with_alpha(alpha),
                ))),
                Transform::from_xyz(0.0, top_y + (1.0 - i as f32) * layout::FLOOR_GAP, Z_FOG),
            ))
            .id();
        commands.entity(root).add_child(band);
    }

    // HUD overlay (Bevy UI, screen-space, on top of the world scene).
    commands
        .spawn((
            MapScene,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn(Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(12.)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|bar| {
                bar.spawn(theme::text(
                    &font,
                    format!("The Barrow Road — Floor {cur_floor}/{}", helheim_core::map::MAP_FLOORS),
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
            root.spawn((
                Node { padding: UiRect::all(Val::Px(12.)), ..default() },
                theme::text(&font, "Click or use ←/→ + Enter to travel", 14., theme::TEXT_DIM),
            ));
        });
}

pub fn despawn_map(
    mut commands: Commands,
    q: Query<Entity, With<MapScene>>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
    // Reset the camera so UI-based screens aren't offset.
    if let Ok(mut cam) = cameras.single_mut() {
        cam.translation.x = 0.0;
        cam.translation.y = 0.0;
    }
}

pub fn draw_edges() {}
```

- [ ] **Step 2: Resolve API mismatches against Bevy 0.18**

Run: `cargo build -p helheim`
Likely adjustments the compiler will point to (fix as needed — these are the known-volatile spots):
- `cameras.single_mut()` may be `cameras.get_single_mut()` depending on the 0.18 point release; use whichever the compiler accepts.
- `Pickable::IGNORE` is the marker that lets clicks pass through the HUD to the world; if the path differs, import from `bevy::picking::Pickable` or drop it (the HUD doesn't overlap nodes, so it's a safety net, not essential).
- `add_children(&[..])` / `add_child(..)` are the 0.18 child-builder calls; adjust if renamed.

Expected after fixes: clean build.

- [ ] **Step 3: Manual checkpoint — static scene renders**

Run: `cargo run -p helheim --features dev`
Then: from the menu, start a run to reach the map.
Expected to SEE:
- Nodes laid out in floors with **distinct tinted icons** per kind (swords/skull/campfire/chest/crown), floor 1 at the bottom, boss at the top.
- The current node ringed in red; reachable nodes brighter with a soft glow disc; lower floors dimmed.
- Fog darkening the very top (boss area) when you're low on the map.
- HUD top bar ("The Barrow Road — Floor n/15", "HP n/m") and the bottom hint.
- No panic; icons appear within a frame (or white placeholder discs if you used those).

(Nodes spawn at `scale ZERO` — they will be invisible until Task 8 adds the reveal animation. To verify Task 6 alone, temporarily change `.with_scale(Vec3::ZERO)` to `.with_scale(Vec3::ONE)`, confirm the scene, then revert it before Task 8.)

- [ ] **Step 4: Commit**

```bash
git add crates/helheim/src/screens/map/scene.rs
git commit -m "feat(map): world-space scene — nodes, icons, glow, fog, HUD"
```

---

## Task 7: Draw the trail edges (gizmos)

Edges are bézier curves sampled via `layout::bezier_points`, trimmed to start/end at the node rim (so they read cleanly regardless of gizmo draw order), colored by `EdgeStyle`.

**Files:**
- Modify: `crates/helheim/src/screens/map/scene.rs`

- [ ] **Step 1: Implement `draw_edges`**

Replace the empty `pub fn draw_edges() {}` with:

```rust
pub fn draw_edges(mut gizmos: Gizmos, session: Res<Session>) {
    let run = &session.run;
    let reachable = run.available_nodes();
    let current = run.position;

    for node in run.map.all() {
        let from = layout::node_pos(node.id);
        for &to_id in &node.next {
            let to = layout::node_pos(to_id);
            let style = layout::edge_style(node.id, to_id, current, &reachable);
            let color = match style {
                layout::EdgeStyle::Available => Color::srgb(0.85, 0.62, 0.30),
                layout::EdgeStyle::Taken => Color::srgb(0.42, 0.32, 0.22),
                layout::EdgeStyle::Ahead => Color::srgb(0.22, 0.22, 0.28),
            };
            // Sample the curve, dropping points inside either node's rim so the
            // line runs rim-to-rim with a small gap at each node.
            let pts: Vec<Vec2> = layout::bezier_points(from, to, 24)
                .into_iter()
                .filter(|p| {
                    p.distance(from) > layout::NODE_R + 2.0 && p.distance(to) > layout::NODE_R + 2.0
                })
                .collect();
            gizmos.linestrip_2d(pts, color);
        }
    }
}
```

- [ ] **Step 2: Add the `EdgeStyle` import**

Ensure the `use super::layout::{...}` line in `scene.rs` brings in what's needed; `layout::EdgeStyle` is referenced fully-qualified above, so no change is required beyond the existing `use super::layout::{self, NodeVisual};`.

- [ ] **Step 3: Build**

Run: `cargo build -p helheim`
Expected: clean build. (If `linestrip_2d` wants `impl IntoIterator<Item = Vec2>` and rejects `Vec<Vec2>`, pass `pts.iter().copied()` or `pts` per the compiler.)

- [ ] **Step 4: Manual checkpoint — paths drawn**

Run: `cargo run -p helheim --features dev` → reach the map.
Expected to SEE: curved lines connecting nodes between floors; the edges leaving your current node are brightest (gold), already-climbed road is muted brown, and the road ahead is dim. Lines stop at node rims (small gap, not stabbing into the icons).

- [ ] **Step 5: Commit**

```bash
git add crates/helheim/src/screens/map/scene.rs
git commit -m "feat(map): curved gizmo trail edges styled by reachability"
```

---

## Task 8: Animations — reveal, pulse, hover, camera follow

**Files:**
- Modify: `crates/helheim/src/screens/map/motion.rs`

- [ ] **Step 1: Implement the four cosmetic systems + camera follow**

Replace the stub `motion.rs` entirely:

```rust
use bevy::prelude::*;

use super::layout;
use super::{GlowAura, MapNodeEnt, Reveal};
use crate::Session;

/// Entrance: scale each node 0 → 1, staggered by floor (bottom-first), then drop `Reveal`.
pub fn reveal_nodes(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &MapNodeEnt, &mut Reveal, &mut Transform)>,
) {
    for (e, node, mut reveal, mut tf) in &mut q {
        // Stagger: higher floors start a little later.
        let delay = node.0.floor as f32 * 0.025;
        reveal.timer.tick(time.delta());
        let raw = reveal.timer.elapsed_secs() - delay;
        let t = (raw / reveal.timer.duration().as_secs_f32()).clamp(0.0, 1.0);
        // ease-out-back-ish: overshoot slightly then settle.
        let s = t * t * (3.0 - 2.0 * t);
        tf.scale = Vec3::splat(s);
        if reveal.timer.finished() {
            tf.scale = Vec3::ONE;
            commands.entity(e).remove::<Reveal>();
        }
    }
}

/// Reachable glow discs breathe (scale + alpha) on a sine.
pub fn pulse_auras(
    time: Res<Time>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut q: Query<(&mut Transform, &MeshMaterial2d<ColorMaterial>), With<GlowAura>>,
) {
    let phase = (time.elapsed_secs() * 2.2).sin() * 0.5 + 0.5; // 0..1
    for (mut tf, mat) in &mut q {
        let s = 0.92 + 0.18 * phase;
        tf.scale = Vec3::splat(s);
        if let Some(m) = materials.get_mut(&mat.0) {
            let base = m.color.alpha();
            // keep hue, breathe alpha around its spawn value
            m.color.set_alpha(0.14 + 0.16 * phase);
            let _ = base;
        }
    }
}

/// Hovered node lifts (scales up); others settle back. Skips nodes still revealing.
pub fn hover_lift(
    time: Res<Time>,
    session: Res<Session>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut q: Query<(&MapNodeEnt, &mut Transform), Without<Reveal>>,
) {
    let hovered = cursor_node(&session, &windows, &cameras);
    let dt = (time.delta_secs() * 12.0).min(1.0);
    for (node, mut tf) in &mut q {
        let target = if Some(node.0) == hovered { 1.14 } else { 1.0 };
        let s = tf.scale.x + (target - tf.scale.x) * dt;
        tf.scale = Vec3::splat(s);
    }
}

/// Camera eases toward the current floor's framing target.
pub fn follow_camera(
    time: Res<Time>,
    session: Res<Session>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let floor = session.run.position.map(|p| p.floor).unwrap_or(0);
    let target = layout::camera_y_for(floor);
    if let Ok(mut cam) = cameras.single_mut() {
        let dt = (time.delta_secs() * 4.0).min(1.0);
        cam.translation.y += (target - cam.translation.y) * dt;
    }
}

/// Token-travel animation lives in `input.rs` as `travel_token` is wired there;
/// re-export a thin wrapper here to keep the plugin's system tuple tidy.
pub use super::input::travel_token;

/// World-space node under the cursor, if any (shared by hover & click).
pub(super) fn cursor_node(
    session: &Session,
    windows: &Query<&Window>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) -> Option<helheim_core::map::NodeId> {
    let window = windows.iter().next()?;
    let cursor = window.cursor_position()?;
    let (camera, cam_tf) = cameras.iter().next()?;
    let world = camera.viewport_to_world_2d(cam_tf, cursor).ok()?;
    let nodes: Vec<(helheim_core::map::NodeId, Vec2)> = session
        .run
        .map
        .all()
        .iter()
        .map(|n| (n.id, layout::node_pos(n.id)))
        .collect();
    layout::pick_node(world, &nodes)
}
```

**Note on the `travel_token` re-export:** to avoid duplicating the cursor/picking helper, `travel_token` is defined in `input.rs` (Task 9) and re-exported here so `MapPlugin`'s system tuple (which lists `motion::travel_token`) resolves. If you prefer, change the plugin tuple in `mod.rs` to `input::travel_token` and delete the `pub use` line — either works.

- [ ] **Step 2: Build (expect one unresolved ref until Task 9)**

Run: `cargo build -p helheim`
Expected: a single error that `input::travel_token` doesn't exist yet. That's fine — it's implemented in Task 9. To verify Task 8 in isolation, temporarily comment out the `pub use super::input::travel_token;` line **and** the `motion::travel_token` entry in `mod.rs`'s system tuple, build, do the manual check, then restore both before Task 9.

- [ ] **Step 3: Manual checkpoint — motion**

With the temporary edits from Step 2, run `cargo run -p helheim --features dev` → reach the map.
Expected to SEE: nodes **fade/scale in bottom-first** when the screen opens; reachable nodes' glow **breathes**; **hovering** a node makes it pop; the camera sits framed on your floor (panning is exercised in Task 9 when you travel).

- [ ] **Step 4: Resolve volatile APIs**

Confirm against the compiler: `m.color.alpha()` / `set_alpha(..)` (Bevy `Color` alpha accessors), `viewport_to_world_2d(..) -> Result<Vec2, _>` (use `.ok()`), `cameras.single_mut()`/`get_single_mut()`. Adjust to whatever 0.18 accepts.

- [ ] **Step 5: Commit**

```bash
git add crates/helheim/src/screens/map/motion.rs crates/helheim/src/screens/map/mod.rs
git commit -m "feat(map): reveal, pulse, hover, and camera-follow animations"
```

---

## Task 9: Input + token travel — the full navigation loop

`navigate` reads mouse (cursor→`pick_node`) and keyboard (`MapSelection` + `keyboard_step`), and on confirm starts a `Traveling` animation. `travel_token` walks the token along the curve and, on completion, calls `run.enter_node` and routes to the next screen — exactly the routing the old screen did ([`map.rs:176-183`](../../../crates/helheim/src/screens/map.rs#L176-L183) in git history).

**Files:**
- Modify: `crates/helheim/src/screens/map/input.rs`
- Modify: `crates/helheim/src/screens/map/scene.rs` (spawn the token entity)

- [ ] **Step 1: Spawn the player token in `spawn_map`**

In `scene.rs`, just before the HUD spawn, add the token at the current node (or at floor-0 origin if no position yet):

```rust
    // Player token — walks the road during travel.
    let token_pos = current.map(layout::node_pos).unwrap_or(Vec2::new(0.0, layout::FLOOR_GAP));
    let token = commands
        .spawn((
            super::PlayerToken,
            Mesh2d(meshes.add(Circle::new(9.0))),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(theme::ACCENT))),
            Transform::from_xyz(token_pos.x, token_pos.y, 4.0),
        ))
        .id();
    commands.entity(root).add_child(token);
```

- [ ] **Step 2: Implement `input.rs`**

```rust
use bevy::prelude::*;
use helheim_core::map::NodeKind;

use super::layout;
use super::motion::cursor_node;
use super::{MapSelection, PlayerToken, Traveling};
use crate::anim::PendingEvents;
use crate::{AppState, Session};

const TRAVEL_SECS: f32 = 0.45;

/// Mouse + keyboard → start travel. No-op while a travel is in progress
/// (except a second confirm/click fast-forwards it — handled in `travel_token`).
pub fn navigate(
    mut commands: Commands,
    session: Res<Session>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut selection: Option<ResMut<MapSelection>>,
    traveling: Option<Res<Traveling>>,
) {
    let reachable = session.run.available_nodes();
    if reachable.is_empty() {
        return;
    }

    // Keyboard: move selection.
    if let Some(sel) = selection.as_mut() {
        if keys.just_pressed(KeyCode::ArrowRight) {
            sel.0 = layout::keyboard_step(sel.0, &reachable, 1);
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            sel.0 = layout::keyboard_step(sel.0, &reachable, -1);
        }
    }

    // While traveling, only a confirm/click matters (to skip) — defer to travel_token.
    if traveling.is_some() {
        return;
    }

    // Determine a confirmed target this frame.
    let mut target = None;
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(node) = cursor_node(&session, &windows, &cameras) {
            if reachable.contains(&node) {
                target = Some(node);
            }
        }
    }
    if keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::ArrowUp)
    {
        if let Some(sel) = selection.as_ref() {
            if reachable.contains(&sel.0) {
                target = Some(sel.0);
            }
        }
    }

    if let Some(to) = target {
        let from = session.run.position.map(layout::node_pos).unwrap_or(layout::node_pos(to));
        commands.insert_resource(Traveling {
            to,
            from,
            timer: Timer::from_seconds(TRAVEL_SECS, TimerMode::Once),
        });
    }
}

/// Walk the token along the curve; on completion enter the node and route.
/// A click or confirm key while traveling fast-forwards (skips) the walk.
#[allow(clippy::too_many_arguments)]
pub fn travel_token(
    time: Res<Time>,
    mut commands: Commands,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<AppState>>,
    mut pending: ResMut<PendingEvents>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    traveling: Option<ResMut<Traveling>>,
    mut tokens: Query<&mut Transform, With<PlayerToken>>,
) {
    let Some(mut trav) = traveling else { return };

    // Skip on a second confirm/click.
    if mouse.just_pressed(MouseButton::Left)
        || keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
    {
        let dur = trav.timer.duration();
        trav.timer.set_elapsed(dur);
    }

    trav.timer.tick(time.delta());
    let t = trav.timer.fraction();
    let to_pos = layout::node_pos(trav.to);
    let p = layout::bezier_point_at(trav.from, to_pos, t);
    if let Ok(mut tf) = tokens.single_mut() {
        tf.translation.x = p.x;
        tf.translation.y = p.y;
    }

    if !trav.timer.finished() {
        return;
    }

    // Arrived: commit to the core and route, mirroring the original screen.
    let to = trav.to;
    let kind = session.run.map.node(to).kind;
    match session.run.enter_node(to) {
        Ok(events) => match kind {
            NodeKind::Monster | NodeKind::Elite | NodeKind::Boss => {
                pending.0 = events;
                next.set(AppState::Combat);
            }
            NodeKind::Treasure => next.set(AppState::Reward),
            NodeKind::Rest => next.set(AppState::Rest),
        },
        Err(err) => warn!("rejected node {to:?}: {err:?}"),
    }
    commands.remove_resource::<Traveling>();
}
```

- [ ] **Step 3: Restore the `mod.rs` / `motion.rs` wiring**

Undo any temporary comment-outs from Task 8 Step 2 so both `motion::travel_token` (the re-export) is live and listed in the `MapPlugin` Update tuple.

- [ ] **Step 4: Build**

Run: `cargo build -p helheim`
Expected: clean build. (Volatile spots: `tokens.single_mut()`/`get_single_mut()`, `Timer::set_elapsed(Duration)` — pass `trav.timer.duration()`.)

- [ ] **Step 5: Manual checkpoint — full loop**

Run: `cargo run -p helheim --features dev` → reach the map.
Verify ALL of:
- Click a reachable node → token **walks the curved road** to it, then the correct screen opens (fight/rest/treasure).
- Use **←/→** to move the highlighted selection; **Enter/Space/↑** travels to it.
- Clicking again mid-walk **skips** to the end (snappy).
- After a fight, returning to the map shows you advanced a floor and the **camera has panned up**.
- Clicking a non-reachable node does nothing.

- [ ] **Step 6: Commit**

```bash
git add crates/helheim/src/screens/map/input.rs crates/helheim/src/screens/map/scene.rs crates/helheim/src/screens/map/mod.rs crates/helheim/src/screens/map/motion.rs
git commit -m "feat(map): mouse + keyboard navigation with token-travel animation"
```

---

## Task 10: Integration — lint, tests, full play-test, docs

**Files:**
- Modify: `README.md` (controls)

- [ ] **Step 1: Update the README controls line**

In `README.md`, replace the Map controls bullet:

```markdown
- **Map:** click a highlighted node to travel, or use ←/→ to pick a node and Enter/↑ to travel; your token walks the road
```

- [ ] **Step 2: Full workspace test**

Run: `cargo test --workspace`
Expected: PASS, including the 8 `layout::tests` and the `map::tests::map_assets_load_at_plugin_build` smoke test, and all unchanged core tests.

- [ ] **Step 3: Lint at the project's CI bar**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings. (If `travel_token`'s arg count trips a lint despite `#[allow]`, keep the allow; if any unused-import warnings appear from the refactor, remove them.)

- [ ] **Step 4: Full manual play-test checklist (the spec's GUI acceptance)**

Run: `cargo run -p helheim -- --seed 7` (reproducible) and climb a full run. Confirm:
- [ ] Icons render and tint per kind (swords/skull/campfire/chest/crown).
- [ ] Curved trail paths drawn; available road brightest, ahead dim.
- [ ] Reachable nodes pulse; hovered node lifts.
- [ ] Token travels the road on select; travel is skippable.
- [ ] Camera pans while climbing and clamps at floor 1 and the boss.
- [ ] Mouse **and** keyboard both navigate.
- [ ] Fog over unreached (top) floors.
- [ ] No clipping across all 16 floors in the 720px window (the deferred gap, closed).
- [ ] Reaching the boss and winning/losing still routes correctly.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: map controls reflect keyboard navigation"
```

- [ ] **Step 6: Update the project memory**

The deferred map-screen UX gaps recorded in `phase-2-progress` memory (no edges, mouse-only, 720px clipping) are now closed. Note that in the memory file so future sessions don't re-flag them.

---

## Self-Review

**Spec coverage:**
- World-space rebuild (Approach B) → Tasks 4, 6 (scene), architecture in header. ✓
- `screens/map/` module split, pure tested `layout.rs` → Tasks 1–4. ✓
- Per-kind tinted icon sprites + free CC BY set + CREDITS → Tasks 5, 6. ✓
- Curved bézier trails, styled by reachability → Tasks 1, 3, 7. ✓
- Four animations (token travel, reachable pulse, hover lift + road shimmer, entrance reveal) → Tasks 8, 9. **Note:** "road shimmer" (animated dashes on ahead-edges) was simplified to static styled edges in Task 7 to keep gizmo edges cheap; the pulse/hover/travel/reveal motions are all present. If shimmer is wanted, it's an additive tweak to `draw_edges` (offset a dash pattern by `time.elapsed_secs()`). Flagged here rather than silently dropped.
- Following, clamped camera → Tasks 1 (`camera_y_for`), 6 (framing), 8 (follow). ✓
- Fog over unreached floors → Task 6 (translucent quads instead of a PNG gradient — documented deviation). ✓
- Mouse + keyboard navigation funneling into one travel routine → Task 9. ✓
- Unit tests for pure layout math + headless smoke test → Tasks 1–4, 10. ✓
- `helheim_core` untouched → no task modifies it. ✓
- Closes the three deferred gaps → Task 10 checklist + memory update. ✓

**Placeholder scan:** No "TBD"/"handle errors"/"similar to Task N". The two `{}` stubs in Task 4 are intentional, named, and replaced in Tasks 6–9. Volatile-API notes give concrete fallbacks, not vague directions.

**Type consistency:** `MapAssets`, `MapScene`, `MapNodeEnt`, `PlayerToken`, `GlowAura`, `Reveal`, `MapSelection`, `Traveling` are defined once in `mod.rs` (Task 4) and used with the same fields/signatures in Tasks 6–9. `Traveling { to, from, timer }`, `node_pos`, `bezier_point_at`, `camera_y_for`, `pick_node`, `keyboard_step`, `node_visual`, `edge_style`, `kind_color`, `MapAssets::for_kind` are referenced exactly as defined. The plugin's Update tuple names match the system fn names (with `travel_token` re-exported from `input` through `motion`).

**Decisions flagged for the implementer:** (1) road-shimmer simplified — see above; (2) glow/fog via meshes not PNGs — documented; (3) visual classifiers placed in `layout.rs` — documented.
