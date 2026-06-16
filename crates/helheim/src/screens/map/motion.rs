use bevy::prelude::*;

use super::layout;
use super::{GlowAura, MapNodeEnt, Reveal, Traveling};
use crate::Session;

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

/// Camera eases toward the floor in focus — the destination while traveling,
/// else the current floor — so it pans up as you climb.
pub fn follow_camera(
    time: Res<Time>,
    session: Res<Session>,
    traveling: Option<Res<Traveling>>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let floor = traveling
        .map(|t| t.to.floor)
        .or_else(|| session.run.position.map(|p| p.floor))
        .unwrap_or(0);
    let target = layout::camera_y_for(floor);
    if let Ok(mut cam) = cameras.single_mut() {
        let dt = (time.delta_secs() * 4.0).min(1.0);
        cam.translation.y += (target - cam.translation.y) * dt;
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

/// Implemented in Task 9: walks the token along the curve and routes on arrival.
pub fn travel_token() {}
