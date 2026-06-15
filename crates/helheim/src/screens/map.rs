use bevy::prelude::*;
use helheim_core::map::{NodeId, NodeKind, BOSS_FLOOR, MAP_FLOORS};

use crate::anim::PendingEvents;
use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Map), spawn_map)
            .add_systems(OnExit(AppState::Map), despawn_map)
            .add_systems(Update, node_buttons.run_if(in_state(AppState::Map)));
    }
}

#[derive(Component)]
struct MapRoot;

#[derive(Component, Clone, Copy)]
struct NodeButton(NodeId);

fn icon(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Monster => "Fight",
        NodeKind::Elite => "ELITE",
        NodeKind::Rest => "Rest",
        NodeKind::Treasure => "Loot",
        NodeKind::Boss => "BOSS",
    }
}

fn spawn_map(mut commands: Commands, session: Res<Session>, font: Res<UiFont>) {
    let run = &session.run;
    let reachable: Vec<NodeId> = run.available_nodes();
    let floor = run.position.map(|p| p.floor).unwrap_or(0);

    commands
        .spawn((
            MapRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| {
            // top bar
            root.spawn(Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(12.)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|bar| {
                bar.spawn(theme::text(
                    &font,
                    format!("The Barrow Road — Floor {floor}/{MAP_FLOORS}"),
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

            // floors, boss (16) at the top down to floor 1 at the bottom
            root.spawn(Node {
                width: Val::Percent(100.),
                flex_grow: 1.,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(6.),
                padding: UiRect::all(Val::Px(8.)),
                ..default()
            })
            .with_children(|col| {
                for f in (1..=BOSS_FLOOR).rev() {
                    col.spawn(Node {
                        column_gap: Val::Px(10.),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        let mut nodes: Vec<_> = run
                            .map
                            .nodes_on(f)
                            .into_iter()
                            .map(|n| (n.id, n.kind))
                            .collect();
                        nodes.sort_by_key(|(id, _)| id.col);
                        for (id, kind) in nodes {
                            let is_reachable = reachable.contains(&id);
                            let is_here = run.position == Some(id);
                            let bg = if is_here {
                                theme::ACCENT
                            } else if is_reachable {
                                theme::PANEL_HOVER
                            } else {
                                theme::PANEL_DIM
                            };
                            row.spawn((
                                NodeButton(id),
                                Button,
                                Node {
                                    width: Val::Px(76.),
                                    height: Val::Px(34.),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(bg),
                            ))
                            .with_children(|b| {
                                let color = if is_reachable || is_here {
                                    theme::TEXT
                                } else {
                                    theme::TEXT_DIM
                                };
                                b.spawn(theme::text(&font, icon(kind), 15., color));
                            });
                        }
                    });
                }
            });

            root.spawn((
                Node {
                    padding: UiRect::all(Val::Px(10.)),
                    ..default()
                },
                Text::new("Click a highlighted node to travel"),
                TextFont {
                    font: font.0.clone(),
                    font_size: 14.,
                    ..default()
                },
                TextColor(theme::TEXT_DIM),
            ));
        });
}

fn despawn_map(mut commands: Commands, q: Query<Entity, With<MapRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
fn node_buttons(
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<AppState>>,
    mut pending: ResMut<PendingEvents>,
    buttons: Query<(&Interaction, &NodeButton), Changed<Interaction>>,
) {
    let reachable = session.run.available_nodes();
    for (interaction, btn) in &buttons {
        if *interaction == Interaction::Pressed && reachable.contains(&btn.0) {
            use helheim_core::map::NodeKind::*;
            let kind = session.run.map.node(btn.0).kind;
            // The reachability guard above should make this infallible; if the
            // core ever rejects it, stay consistent and skip rather than panic.
            let events = match session.run.enter_node(btn.0) {
                Ok(events) => events,
                Err(err) => {
                    warn!("rejected node {:?}: {err:?}", btn.0);
                    return;
                }
            };
            match kind {
                Monster | Elite | Boss => {
                    pending.0 = events;
                    next.set(AppState::Combat);
                }
                Treasure => next.set(AppState::Reward),
                Rest => next.set(AppState::Rest),
            }
            return;
        }
    }
}
