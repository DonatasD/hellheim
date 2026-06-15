use bevy::prelude::*;
use helheim_core::run::Stage;

use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub struct RewardPlugin;

impl Plugin for RewardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Reward), spawn_reward)
            .add_systems(OnExit(AppState::Reward), despawn_reward)
            .add_systems(Update, reward_clicks.run_if(in_state(AppState::Reward)));
    }
}

#[derive(Component)]
struct RewardRoot;

#[derive(Component)]
struct RewardButton(usize);

#[derive(Component)]
struct SkipButton;

fn spawn_reward(mut commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    let Stage::Reward { offer, .. } = session.run.stage else {
        return;
    };
    commands
        .spawn((
            RewardRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(30.),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(theme::text(&font, "Claim your spoils", 40., theme::ACCENT));
            root.spawn(Node {
                column_gap: Val::Px(20.),
                ..default()
            })
            .with_children(|row| {
                for (i, card) in offer.iter().enumerate() {
                    let spec = card.spec();
                    row.spawn((
                        RewardButton(i),
                        Button,
                        Node {
                            width: Val::Px(190.),
                            height: Val::Px(230.),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(14.)),
                            ..default()
                        },
                        BackgroundColor(theme::PANEL),
                    ))
                    .with_children(|c| {
                        c.spawn(theme::text(
                            &font,
                            format!("({}) {}", spec.cost, spec.name),
                            20.,
                            theme::TEXT,
                        ));
                        c.spawn(theme::text(&font, spec.text, 16., theme::TEXT_DIM));
                    });
                }
            });
            theme::button(root, &font, SkipButton, "Walk on (skip)");
        });
}

fn despawn_reward(mut commands: Commands, q: Query<Entity, With<RewardRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn reward_clicks(
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<AppState>>,
    cards: Query<(&Interaction, &RewardButton), Changed<Interaction>>,
    skips: Query<&Interaction, (Changed<Interaction>, With<SkipButton>)>,
) {
    for (interaction, button) in &cards {
        if *interaction == Interaction::Pressed && session.run.choose_reward(Some(button.0)).is_ok()
        {
            next.set(AppState::Map);
            return;
        }
    }
    for interaction in &skips {
        if *interaction == Interaction::Pressed && session.run.choose_reward(None).is_ok() {
            next.set(AppState::Map);
            return;
        }
    }
}
