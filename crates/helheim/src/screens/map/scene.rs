use bevy::prelude::*;
use helheim_core::map::BOSS_FLOOR;

use super::layout::{self, NodeVisual};
use super::{GlowAura, MapAssets, MapNodeEnt, MapScene, MapSelection, Reveal};
use crate::theme::{self, UiFont};
use crate::Session;

const Z_GLOW: f32 = 1.0;
const Z_RING: f32 = 1.8;
const Z_BODY: f32 = 2.0;
const Z_ICON: f32 = 3.0;
const Z_FOG: f32 = 5.0;

/// Dim a color toward black by `factor` (keeps alpha).
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
    let ring_circle = meshes.add(Circle::new(layout::NODE_R + 4.0));
    let glow_circle = meshes.add(Circle::new(layout::NODE_R * 1.7));

    let root = commands
        .spawn((MapScene, Transform::default(), Visibility::Visible))
        .id();

    for node in run.map.all() {
        let id = node.id;
        let pos = layout::node_pos(id);
        let vis = layout::node_visual(id, current, &reachable);
        let base = layout::kind_color(node.kind);

        let (ring_color, icon_color) = match vis {
            NodeVisual::Current => (theme::ACCENT, Color::WHITE),
            NodeVisual::Reachable => (base, Color::WHITE),
            NodeVisual::Visited => (dim(base, 0.35), dim(Color::WHITE, 0.4)),
            NodeVisual::Locked => (dim(base, 0.25), dim(Color::WHITE, 0.3)),
        };

        // Body (parent) — dark disc, starts invisible (scale 0) for the reveal.
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
                Mesh2d(ring_circle.clone()),
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

        // Glow behind reachable/current nodes (separate sibling — pulses independently).
        if matches!(vis, NodeVisual::Reachable | NodeVisual::Current) {
            let glow = commands
                .spawn((
                    GlowAura,
                    Mesh2d(glow_circle.clone()),
                    MeshMaterial2d(materials.add(ColorMaterial::from_color(base.with_alpha(0.22)))),
                    Transform::from_xyz(pos.x, pos.y, Z_GLOW),
                ))
                .id();
            commands.entity(root).add_child(glow);
        }
    }

    // Fog: three stacked translucent dark quads above the boss, fading downward.
    let fog_w = layout::COL_GAP * 8.0;
    let top_y = BOSS_FLOOR as f32 * layout::FLOOR_GAP;
    let fog_rect = meshes.add(Rectangle::new(fog_w, layout::FLOOR_GAP));
    for (i, alpha) in [0.85_f32, 0.55, 0.25].into_iter().enumerate() {
        let band = commands
            .spawn((
                Mesh2d(fog_rect.clone()),
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
    commands.remove_resource::<MapSelection>();
    for e in &q {
        commands.entity(e).despawn();
    }
    if let Ok(mut cam) = cameras.single_mut() {
        cam.translation.x = 0.0;
        cam.translation.y = 0.0;
    }
}

pub fn draw_edges() {}
