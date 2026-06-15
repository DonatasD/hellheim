use bevy::prelude::*;
use helheim_core::run::RunState;

use crate::theme::{self, UiFont};
use crate::{AppState, CliSeed, Session};

pub struct EndScreensPlugin;

impl Plugin for EndScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Victory), spawn_victory)
            .add_systems(OnEnter(AppState::GameOver), spawn_game_over)
            .add_systems(OnExit(AppState::Victory), despawn_end)
            .add_systems(OnExit(AppState::GameOver), despawn_end)
            .add_systems(
                Update,
                end_clicks.run_if(in_state(AppState::Victory).or(in_state(AppState::GameOver))),
            );
    }
}

#[derive(Component)]
struct EndRoot;

#[derive(Component)]
struct AgainButton;

#[derive(Component)]
struct MenuButton;

fn spawn_victory(commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    spawn_end(
        commands,
        &session,
        &font,
        "THE BARROW ROAD IS CLEARED",
        theme::ENERGY_COLOR,
    );
}

fn spawn_game_over(commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    spawn_end(
        commands,
        &session,
        &font,
        "SLAIN ON THE BARROW ROAD",
        theme::ACCENT,
    );
}

fn spawn_end(
    mut commands: Commands,
    session: &Session,
    font: &UiFont,
    title: &str,
    title_color: Color,
) {
    let run = &session.run;
    let stats = [
        format!("Turns taken: {}", run.stats.turns),
        format!("Damage dealt: {}", run.stats.damage_dealt),
        format!("Damage taken: {}", run.stats.damage_taken),
        format!("Final deck: {} cards", run.master_deck.len()),
        format!("Seed: {}", run.seed),
    ];
    commands
        .spawn((
            EndRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(14.),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(theme::text(font, title, 52., title_color));
            for line in stats {
                root.spawn(theme::text(font, line, 22., theme::TEXT));
            }
            root.spawn(Node {
                column_gap: Val::Px(16.),
                margin: UiRect::top(Val::Px(20.)),
                ..default()
            })
            .with_children(|row| {
                theme::button(row, font, AgainButton, "Descend Again");
                theme::button(row, font, MenuButton, "Back to Menu");
            });
        });
}

fn despawn_end(mut commands: Commands, q: Query<Entity, With<EndRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn end_clicks(
    mut commands: Commands,
    cli: Res<CliSeed>,
    mut next: ResMut<NextState<AppState>>,
    again: Query<&Interaction, (Changed<Interaction>, With<AgainButton>)>,
    menu: Query<&Interaction, (Changed<Interaction>, With<MenuButton>)>,
) {
    for interaction in &again {
        if *interaction == Interaction::Pressed {
            commands.insert_resource(Session {
                run: RunState::new(cli.next_seed()),
            });
            next.set(AppState::Map);
            return;
        }
    }
    for interaction in &menu {
        if *interaction == Interaction::Pressed {
            next.set(AppState::Menu);
            return;
        }
    }
}
