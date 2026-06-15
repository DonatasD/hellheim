use bevy::prelude::*;

use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub struct RestPlugin;

impl Plugin for RestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Rest), spawn_rest)
            .add_systems(OnExit(AppState::Rest), despawn_rest)
            .add_systems(Update, continue_button.run_if(in_state(AppState::Rest)));
    }
}

#[derive(Component)]
struct RestRoot;

#[derive(Component)]
struct ContinueButton;

fn spawn_rest(mut commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    let hp = session.run.hp;
    let max = session.run.max_hp;
    commands
        .spawn((
            RestRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(24.),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(theme::text(
                &font,
                "You rest by the fire",
                40.,
                theme::ACCENT,
            ));
            root.spawn(theme::text(
                &font,
                format!("HP {hp}/{max}"),
                24.,
                theme::HP_COLOR,
            ));
            theme::button(root, &font, ContinueButton, "Continue");
        });
}

fn despawn_rest(mut commands: Commands, q: Query<Entity, With<RestRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn continue_button(
    mut next: ResMut<NextState<AppState>>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            next.set(AppState::Map);
        }
    }
}
