use bevy::prelude::*;
use helheim_core::run::RunState;

use crate::theme::{self, UiFont};
use crate::{AppState, CliSeed, Session};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), spawn_menu)
            .add_systems(OnExit(AppState::Menu), despawn_menu)
            .add_systems(Update, begin_button.run_if(in_state(AppState::Menu)));
    }
}

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct BeginButton;

fn spawn_menu(mut commands: Commands, font: Res<UiFont>) {
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(28.),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn(theme::text(&font, "HELHEIM", 80., theme::ACCENT));
            p.spawn(theme::text(&font, "the barrow road", 26., theme::TEXT_DIM));
            theme::button(p, &font, BeginButton, "Begin the Descent");
        });
}

fn despawn_menu(mut commands: Commands, q: Query<Entity, With<MenuRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
fn begin_button(
    mut commands: Commands,
    cli: Res<CliSeed>,
    mut q: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<BeginButton>)>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, mut bg) in &mut q {
        match interaction {
            Interaction::Pressed => {
                commands.insert_resource(Session {
                    run: RunState::new(cli.next_seed()),
                });
                next.set(AppState::Combat);
            }
            Interaction::Hovered => *bg = BackgroundColor(theme::PANEL_HOVER),
            Interaction::None => *bg = BackgroundColor(theme::PANEL),
        }
    }
}
