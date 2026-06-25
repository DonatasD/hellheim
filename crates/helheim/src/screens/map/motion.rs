use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::layout;
use super::{CameraTarget, GlowAura, MapNodeEnt, PlayerToken, Reveal, Traveling};
use crate::anim::PendingEvents;
use crate::AppState;
use crate::Session;
use helheim_core::map::NodeKind;

/// Entrance: scale each node 0 → 1, staggered by floor (bottom-first), then drop `Reveal`.
pub fn reveal_nodes(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &MapNodeEnt, &mut Reveal, &mut Transform)>,
) {
    for (e, node, mut reveal, mut tf) in &mut q {
        let delay = node.0.floor as f32 * 0.025;
        reveal.timer.tick(time.delta());
        let raw = reveal.timer.elapsed_secs() - delay;
        let t = (raw / reveal.timer.duration().as_secs_f32()).clamp(0.0, 1.0);
        let s = t * t * (3.0 - 2.0 * t); // smoothstep
        tf.scale = Vec3::splat(s);
        if reveal.timer.is_finished() {
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
        tf.scale = Vec3::splat(0.92 + 0.18 * phase);
        if let Some(m) = materials.get_mut(&mat.0) {
            m.color.set_alpha(0.14 + 0.16 * phase);
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

/// Camera eases toward the focus point: the destination while traveling, else
/// the scroll target (set by entry and `scroll_camera`). This lets the player
/// scroll to preview the map while still auto-recentering on travel and entry.
pub fn follow_camera(
    time: Res<Time>,
    traveling: Option<Res<Traveling>>,
    target: Option<Res<CameraTarget>>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let goal = match (traveling, target) {
        (Some(t), _) => layout::camera_y_for(t.to.floor),
        (None, Some(t)) => t.0,
        (None, None) => return,
    };
    if let Ok(mut cam) = cameras.single_mut() {
        let dt = (time.delta_secs() * 4.0).min(1.0);
        cam.translation.y += (goal - cam.translation.y) * dt;
    }
}

/// Mouse wheel scrolls the camera target up/down within the map bounds so the
/// player can preview floors ahead. Normalizes line- vs pixel-unit scrolling.
pub fn scroll_camera(mut wheel: MessageReader<MouseWheel>, target: Option<ResMut<CameraTarget>>) {
    let Some(mut target) = target else {
        return;
    };
    let mut delta = 0.0;
    let mut scrolled = false;
    for ev in wheel.read() {
        delta += match ev.unit {
            MouseScrollUnit::Line => ev.y * 48.0,
            MouseScrollUnit::Pixel => ev.y * 0.6,
        };
        scrolled = true;
    }
    if scrolled {
        target.0 = layout::clamp_cam_y(target.0 + delta);
    }
}

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

/// Walk the token along the curve; on completion enter the node and route.
/// A click or Enter/Space while traveling fast-forwards (skips) the walk.
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
    let Some(mut trav) = traveling else {
        return;
    };

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

    if !trav.timer.is_finished() {
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
