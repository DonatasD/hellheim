use bevy::prelude::*;

use super::layout;
use super::motion::cursor_node;
use super::{MapSelection, Traveling};
use crate::Session;

const TRAVEL_SECS: f32 = 0.45;

/// Mouse + keyboard → start a travel. No-op while a travel is in progress.
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

    // Keyboard: move the highlighted selection across the reachable set.
    if let Some(sel) = selection.as_mut() {
        if keys.just_pressed(KeyCode::ArrowRight) {
            sel.0 = layout::keyboard_step(sel.0, &reachable, 1);
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            sel.0 = layout::keyboard_step(sel.0, &reachable, -1);
        }
    }

    // While traveling, don't start another (skip is handled in travel_token).
    if traveling.is_some() {
        return;
    }

    // Resolve a confirmed target this frame (mouse click or keyboard confirm).
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
        let from = session
            .run
            .position
            .map(layout::node_pos)
            .unwrap_or_else(|| Vec2::new(0.0, layout::FLOOR_GAP));
        commands.insert_resource(Traveling {
            to,
            from,
            timer: Timer::from_seconds(TRAVEL_SECS, TimerMode::Once),
        });
    }
}
