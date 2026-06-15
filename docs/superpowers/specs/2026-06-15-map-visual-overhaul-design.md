# Helheim — Map Visual Overhaul (Atmospheric World-Space Map) Design

**Status:** Approved design, ready for implementation planning.
**Predecessors:** [`2026-06-14-act1-map-foundation-design.md`](2026-06-14-act1-map-foundation-design.md) (the map this polishes), [`2026-06-10-helheim-design.md`](2026-06-10-helheim-design.md) (overall design).

## Context

Spec 1 shipped the Act 1 branching node map, but its final review flagged three
deferred map-screen UX gaps: **paths/edges aren't drawn** (nodes are colored
buttons, branching isn't conveyed), **navigation is mouse-only** (the spec wanted
keyboard too), and the **16-floor map can clip** in the 720px window. This spec
is a **presentation-layer overhaul** that closes all three gaps and elevates the
map from flat buttons to an atmospheric, animated, world-space scene with
connecting trails, per-kind icons, and a following camera.

This is **polish, not a Phase 2 spec** — it does not advance the spec sequence
(Spec 2 "Gold & Shops" remains next). It touches only the Bevy shell.

## Goal

Rebuild the Act 1 map screen ([`crates/helheim/src/screens/map.rs`](../../../crates/helheim/src/screens/map.rs))
as a **world-space 2D scene**: nodes carry a distinct icon per kind, are joined by
curved trail paths, and the player's token travels the road when a node is chosen.
Reachable nodes pulse, the hovered node lifts, floors reveal on entry, fog hangs
over floors not yet reached, and the camera follows the climb. Both mouse and
keyboard drive navigation. `helheim_core` is **untouched** — the graph already
exposes everything needed.

**Conventions (inherited):** the rules engine stays pure and deterministic; all
animation is cosmetic and time-based, never touching `RunRng` or run state, so a
seed still reproduces a whole run. Pure presentation math is unit-tested.

## Scope

**In scope:** a world-space map renderer (Approach B below); per-kind node icons
from a free sprite set, tinted at runtime; curved bézier edges (solid for
takeable road, dashed/shimmering for the road ahead); the four animations (token
travel, reachable pulse, hover lift + edge shimmer, entrance reveal); a
following, clamped camera; fog over unreached floors; mouse **and** keyboard
navigation; a thin UI HUD overlay (floor/HP, hint); unit tests for the pure
layout math; an asset-attribution `CREDITS.md`.

**Out of scope (YAGNI / later):** custom or hand-painted art (we tint a free
sprite set; richer art can swap in later with no code change); minimap, zoom
controls, node tooltips/encounter previews; any new node kind; any `helheim_core`
change; anything in Spec 2+ (gold, shops, upgrades, events, save).

---

## 1. Architecture & rendering approach

**Approach B — world-space 2D scene** (chosen over pure Bevy UI and a UI/world
hybrid). The app already spawns a global `Camera2d` ([`theme.rs`](../../../crates/helheim/src/theme.rs)).
The map renders in world space (sprites, meshes, gizmos) with a **thin Bevy UI
overlay** for the HUD only.

Why not the alternatives:

- **Pure Bevy UI** can't draw diagonal/curved lines — edges would be
  rotated-rectangle hacks and curves are intractable; camera-follow becomes manual
  scroll math. We'd fight the layout engine for exactly the features requested.
- **Hybrid (UI nodes + world-space lines)** forces constant sync between
  screen-space UI positions and world-space coords across a panning camera — more
  complexity than either pure path.

In world space, every requested feature is natural: curved trails = sampled
bézier → `Gizmos`; token travel = `Transform` lerp along that curve; camera
follow = lerp the camera `Transform.y`; glow/pulse/fog = additive sprites with
animated alpha/scale; tinted icons = `Sprite { image, color }`. Bevy 0.18 also
ships **sprite picking**, so mouse hover/click stays largely free.

UI layout is screen-space and unaffected by the `Camera2d` translation, so moving
the camera for the map does not disturb the (UI-based) combat/reward/rest screens.

## 2. Module layout

[`screens/map.rs`](../../../crates/helheim/src/screens/map.rs) (188 lines today)
would exceed ~600 lines with sprites, glow, fog, four animations, picking,
keyboard, and camera in one file. Promote it to a `screens/map/` module, each
file single-purpose:

| File | Responsibility |
|---|---|
| `mod.rs` | `MapPlugin`, system-set wiring, screen lifecycle (spawn/despawn), shared components & resources |
| `layout.rs` | **Pure, unit-tested** geometry — no Bevy queries |
| `scene.rs` | Spawn node entities (ring + body + tinted icon + glow), fog, UI HUD; draw edges via gizmos |
| `motion.rs` | The four animations + camera-follow |
| `input.rs` | Mouse picking + keyboard selection → `enter_node` routing |

## 3. Layout geometry (`layout.rs`, pure + tested)

World coordinates: y-up, origin centered (Bevy `Camera2d` default). Floor 1 sits
lowest, the boss (floor 16) highest. Columns (`MAP_WIDTH = 7`) are centered.

```rust
pub const FLOOR_GAP: f32 = 120.0;   // vertical world units between floors
pub const COL_GAP:   f32 = 110.0;   // horizontal world units between columns
pub const NODE_R:    f32 = 30.0;    // node hit/visual radius

pub fn node_pos(id: NodeId) -> Vec2;                    // (col-centered x, floor y)
pub fn bezier_points(from: Vec2, to: Vec2, n: usize) -> Vec<Vec2>;
                                                        // vertical-tangent cubic, n samples
pub fn camera_y_for(floor: u8) -> f32;                  // follow target, clamped to [floor1, boss]
pub fn pick_node(cursor_world: Vec2, nodes: &[(NodeId, Vec2)]) -> Option<NodeId>;
                                                        // nearest within NODE_R
pub fn keyboard_step(current: NodeId, reachable: &[NodeId], dir: i8) -> NodeId;
                                                        // cycle reachable set by column, wrapping
```

The edge curve is a cubic with vertical tangents at both ends
(`C (from.x, mid_y) (to.x, mid_y) to`), giving the smooth "trail" bend seen in the
mockups; it is crossing-free because the core graph is (Spec 1 guarantees no
crossing edges).

## 4. Visual design (the "Illustrated" look)

**Per-kind color:** Monster = steel/grey, Elite = red (`theme::ACCENT`),
Rest = warm orange, Treasure = gold (`theme::ENERGY_COLOR`), Boss = gold/accent.

**Node states:**

- **Current** (player's position) — accent ring + a small "you are here" token.
- **Reachable** (in `available_nodes()`) — colored ring + a pulsing glow aura.
- **Visited / past floors** — dimmed.
- **Locked / ahead** — dim, low-opacity; floors above the frontier sit under fog.

A node is layered circle meshes (colored ring behind a dark body) with the tinted
icon `Sprite` centered, plus a soft glow sprite behind reachable nodes.

**Edges:** bézier curves between connected nodes. Takeable road (from current to
its children) = solid warm tone; road ahead = dashed with an upward-flowing
shimmer (animated dash phase); far-ahead dimmed.

**Fog:** a gradient overlay near the top of the view, obscuring floors past the
frontier; it lifts as the camera climbs. Reinforces roguelike mystery and is
purely cosmetic.

**HUD (Bevy UI overlay, on top of the scene):** the existing top bar
("The Barrow Road — Floor f/16", "HP n/m") and bottom hint
("Click or use ←/→ + Enter to travel"). Reuses [`theme::text`](../../../crates/helheim/src/theme.rs).

## 5. Icons & assets

Add under [`crates/helheim/assets/`](../../../crates/helheim/assets/):

- `icons/fight.png`, `elite.png`, `rest.png`, `treasure.png`, `boss.png` — white
  silhouette icons (crossed swords / skull / campfire / chest / crown) from
  **game-icons.net (CC BY 3.0)**, tinted per kind at runtime via `Sprite.color`.
- `icons/glow.png` — a soft radial-gradient sprite for the pulse auras.
- Node discs/rings use `Mesh2d` circles (crisp, recolorable, no texture needed).

**Licensing:** game-icons.net is CC BY 3.0 — add a top-level `CREDITS.md`
attributing the icon authors + license, and a one-line pointer from the README.
This is required by the license and must ship with the assets.

**Loading:** icons load once at plugin-build time into a `MapIcons` resource,
mirroring how the font loads in [`theme.rs`](../../../crates/helheim/src/theme.rs#L27-L30)
(the `AssetServer` already exists when `MapPlugin` builds).

```rust
#[derive(Resource)]
pub struct MapIcons {
    pub fight: Handle<Image>, pub elite: Handle<Image>, pub rest: Handle<Image>,
    pub treasure: Handle<Image>, pub boss: Handle<Image>, pub glow: Handle<Image>,
}
```

## 6. Animations & camera

All are time-based (`Res<Time>`) and cosmetic.

1. **Token travel** — on confirming a reachable node, the player token lerps along
   `bezier_points(current, target)` over ~0.45s (eased). Input is locked during
   travel via a `Traveling` resource; on completion the system calls
   `enter_node` and routes. **Skippable:** a second click/confirm fast-forwards the
   timer to done so it never feels slow.
2. **Reachable pulse** — `GlowAura` sprites on reachable nodes oscillate scale +
   alpha (sine over `Time`).
3. **Hover lift + edge shimmer** — the hovered node's icon/body scales ~1.1×
   smoothly; ahead-edges animate their dash phase to flow upward.
4. **Entrance reveal** — on `OnEnter(Map)`, nodes fade/scale in staggered by floor
   (bottom-first), a brief one-shot.

**Camera follow** — a system eases `Camera2d` `Transform.translation.y` toward
`camera_y_for(current_floor)`, clamped so it never overscrolls past floor 1 or the
boss. It pans up after travel. Reset to `0.0` on `OnExit(Map)`.

## 7. Input model

```rust
#[derive(Resource)] pub struct MapSelection(pub NodeId);  // keyboard-highlighted reachable node
```

- **Mouse:** hover and click resolve the node under the cursor via the pure
  `pick_node` helper (§3, cursor→world; Bevy 0.18's sprite-picking backend may
  drive this instead, but `pick_node` is the tested source of truth). Clicking a
  reachable node confirms travel to it; hover also moves `MapSelection` so the two
  input modes stay in sync.
- **Keyboard:** `MapSelection` highlights one reachable node. **←/→** cycle the
  reachable set by column (`keyboard_step`, wrapping); **↑** or **Enter/Space**
  confirm travel to the selection.
- Both paths funnel into one "confirm travel to `NodeId`" routine, so behavior is
  identical regardless of input.

## 8. Data flow & lifecycle

- **`OnEnter(Map)`** — read `Session.run` (graph, `position`, `available_nodes()`);
  spawn the scene (nodes, edges set-up, fog) tagged with a `MapScene` root and the
  UI HUD; snap the camera to the current floor; seed `MapSelection` to the first
  reachable node (lowest column).
- **Per frame** — `scene` draws edges (gizmos) from layout + run state; `motion`
  runs the animations + camera; `input` reads mouse/keyboard.
- **On confirm** (mouse or keyboard, only if target ∈ `available_nodes()`) — insert
  `Traveling { to, from, timer }` (capturing the token's start position), animate the token, then on
  completion call `run.enter_node(to)` and route to `Combat` / `Reward` / `Rest`
  exactly as today ([`map.rs:176-183`](../../../crates/helheim/src/screens/map.rs#L176-L183)).
- **`OnExit(Map)`** — despawn everything under `MapScene` and the HUD; reset the
  camera `Transform`.

```rust
#[derive(Resource)] pub struct Traveling { pub to: NodeId, pub from: Vec2, pub timer: Timer }
#[derive(Component)] pub struct MapScene;        // despawn root
#[derive(Component)] pub struct MapNodeEnt(pub NodeId);
#[derive(Component)] pub struct PlayerToken;
#[derive(Component)] pub struct GlowAura;        // pulsing aura on reachable nodes
#[derive(Component)] pub struct NodeIcon;        // hover-scaled child sprite
```

## 9. Error handling & determinism

- Keep Spec 1's infallible guard: only travel to a node in `available_nodes()`; if
  `enter_node` returns `Err`, `warn!` and skip rather than panic
  ([`map.rs:169-174`](../../../crates/helheim/src/screens/map.rs#L169-L174)).
- Icons not yet loaded: the sprite is invisible for a frame or two until the asset
  resolves — acceptable; node ring/body (meshes) render immediately.
- Camera clamps prevent overscroll on tall maps; fog hides not-yet-laid-out detail.
- Animations never read or mutate `RunRng`/run state — determinism is preserved.

## 10. Testing

**Unit tests (pure `layout.rs`):**

- `node_pos`: y strictly increases with floor; x ordered by column and centered.
- `bezier_points`: first/last samples equal the node centers; sample count honored.
- `camera_y_for`: clamps at the floor-1 and boss extremes; monotonic between.
- `pick_node`: returns the node whose center is within `NODE_R` of a cursor point;
  `None` when outside all.
- `keyboard_step`: cycles the reachable set in column order and wraps at both ends.

**Unchanged:** all `helheim_core` map/run tests.

**Manual play-test checklist** (GUI can only be verified at the display):
icons render and tint per kind; curved paths drawn; reachable nodes pulse; token
travels the road on select (and travel is skippable); camera pans while climbing
and clamps at the ends; mouse **and** keyboard both navigate; fog over unreached
floors; no clipping across all 16 floors in the 720px window.

## 11. Non-goals (YAGNI)

No painted/custom art (tint the free sprite set; swap richer art later with no code
change). No minimap, zoom, or tooltips. No new node kinds. No `helheim_core`
changes. Nothing from Spec 2+ (gold, shops, upgrades, events, save).
