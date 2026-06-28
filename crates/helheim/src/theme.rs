use bevy::prelude::*;

pub const BG: Color = Color::srgb(0.07, 0.07, 0.10);
pub const PANEL: Color = Color::srgb(0.14, 0.14, 0.19);
pub const PANEL_HOVER: Color = Color::srgb(0.22, 0.22, 0.30);
pub const ACCENT: Color = Color::srgb(0.78, 0.22, 0.20);
pub const TEXT: Color = Color::srgb(0.92, 0.90, 0.85);
pub const TEXT_DIM: Color = Color::srgb(0.58, 0.58, 0.58);
pub const HP_COLOR: Color = Color::srgb(0.85, 0.30, 0.28);
pub const BLOCK_COLOR: Color = Color::srgb(0.38, 0.58, 0.92);
pub const ENERGY_COLOR: Color = Color::srgb(0.95, 0.76, 0.22);

#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

pub struct ThemePlugin;

impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        // The initial `OnEnter(Menu)` transition runs before `Startup`
        // (StatesPlugin schedules `StateTransition` before `PreStartup`), so
        // any resource a screen's `OnEnter` needs must exist before any
        // schedule runs. Load the font now, at build time — inserting it in a
        // `Startup` system is too late and `spawn_menu` panics on a missing
        // `UiFont`. `AssetServer` is already present (DefaultPlugins built it).
        let font = app
            .world()
            .resource::<AssetServer>()
            .load("fonts/FiraSans-Regular.ttf");
        app.insert_resource(ClearColor(BG))
            .insert_resource(UiFont(font))
            .add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

/// Text bundle helper: every label in the game goes through this.
pub fn text(
    font: &UiFont,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> (Text, TextFont, TextColor) {
    (
        Text::new(s),
        TextFont {
            font: font.0.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

/// A standard button: panel background, padding, child label.
pub fn button(
    commands: &mut ChildSpawnerCommands,
    font: &UiFont,
    marker: impl Bundle,
    label: &str,
) {
    commands
        .spawn((
            marker,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(24.), Val::Px(10.)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|b| {
            b.spawn(text(font, label, 24., TEXT));
        });
}
