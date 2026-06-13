use bevy::prelude::*;
use helheim_core::run::RunState;

mod anim;
mod screens;
mod theme;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    Combat,
    Reward,
    Victory,
    GameOver,
}

/// Seed from `--seed <n>`, if given. "Again" reuses it (reproducible runs);
/// otherwise every run gets a fresh random seed.
#[derive(Resource)]
pub struct CliSeed(pub Option<u64>);

impl CliSeed {
    pub fn next_seed(&self) -> u64 {
        self.0.unwrap_or_else(rand::random::<u64>)
    }
}

#[derive(Resource)]
pub struct Session {
    pub run: RunState,
}

fn parse_seed() -> Option<u64> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == "--seed")
        .and_then(|w| w[1].parse().ok())
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Helheim".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(CliSeed(parse_seed()))
        .init_state::<AppState>()
        .add_plugins((
            theme::ThemePlugin,
            anim::AnimPlugin,
            screens::menu::MenuPlugin,
            screens::combat::CombatScreenPlugin,
            screens::reward::RewardPlugin,
            screens::end::EndScreensPlugin,
        ))
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    /// Headless smoke test: the state machine wires up and transitions.
    #[test]
    fn app_state_machine_transitions() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<AppState>();
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Combat);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Combat
        );
    }

    /// Regression: the initial `OnEnter(Menu)` transition is scheduled BEFORE
    /// `Startup` (StatesPlugin inserts `StateTransition` before `PreStartup`),
    /// so `spawn_menu`'s `Res<UiFont>` must exist before any schedule runs —
    /// i.e. the font must be inserted at plugin-build time, not in `Startup`.
    /// With the font loaded in `Startup`, the first update panics on a missing
    /// resource. This headless test reproduces that without a window.
    #[test]
    fn menu_screen_builds_without_missing_resources() {
        use crate::screens::menu::MenuPlugin;
        use crate::theme::{ThemePlugin, UiFont};
        use bevy::asset::AssetPlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));
        // DefaultPlugins registers the Font asset (via bevy_text); the minimal
        // set here doesn't, so do it explicitly — ThemePlugin loads a Font.
        app.init_asset::<bevy::text::Font>();
        app.init_state::<AppState>();
        app.insert_resource(CliSeed(Some(0)));
        app.add_plugins((ThemePlugin, MenuPlugin));
        app.update();
        assert!(app.world().contains_resource::<UiFont>());
    }
}
