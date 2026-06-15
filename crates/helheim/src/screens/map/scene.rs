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
