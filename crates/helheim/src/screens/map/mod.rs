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
                    motion::scroll_camera,
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

/// Desired camera y; the wheel scrolls this and `follow_camera` eases toward it.
/// Re-seeded to the current floor on enter, so travel and entry recenter the view.
#[derive(Resource)]
pub struct CameraTarget(pub f32);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemePlugin;
    use crate::{CliSeed, Session};
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
