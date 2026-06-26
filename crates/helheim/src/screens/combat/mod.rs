use bevy::prelude::*;
use helheim_core::cards::Targeting;
use helheim_core::combat::{Action, IntentKind, TargetRef};
use helheim_core::run::Stage;
use helheim_core::statuses::Statuses;

use crate::anim::{queue_empty, DisplayState, EventQueue, PanelTarget, PendingEvents};
use crate::theme::{self, UiFont};
use crate::{AppState, Session};

pub mod hand;

pub struct CombatScreenPlugin;

impl Plugin for CombatScreenPlugin {
    fn build(&self, app: &mut App) {
        let card_assets = hand::CardAssets::load(app.world().resource::<AssetServer>());
        app.insert_resource(card_assets);
        app.init_resource::<PendingCard>()
            .init_resource::<TargetCursor>()
            .add_systems(OnEnter(AppState::Combat), enter_combat)
            .add_systems(OnExit(AppState::Combat), exit_combat)
            .add_systems(
                Update,
                (
                    (card_click, enemy_click, end_turn_button, keyboard)
                        .run_if(in_state(AppState::Combat))
                        .run_if(queue_empty),
                    (hand::reconcile_hand, hand::refresh_affordability, sync_texts, highlight_enemies, post_combat)
                        .run_if(in_state(AppState::Combat)),
                ),
            );
    }
}

/// Hand index of a single-target card waiting for the player to pick an enemy.
#[derive(Resource, Default)]
struct PendingCard(Option<usize>);

/// Keyboard target cursor (index into ds.enemies) while a card is pending.
#[derive(Resource, Default)]
struct TargetCursor(usize);

#[derive(Component)]
struct CombatRoot;

/// One text label bound to one piece of DisplayState.
#[derive(Component)]
enum Bind {
    Turn,
    Piles,
    Energy,
    Hp(TargetRef),
    Block(TargetRef),
    Status(TargetRef),
    Intent(usize),
}

#[derive(Component)]
pub(crate) struct HandRow;

#[derive(Component)]
struct EndTurnButton;

fn enter_combat(
    mut commands: Commands,
    session: Res<Session>,
    mut queue: ResMut<EventQueue>,
    mut pending: ResMut<PendingEvents>,
    font: Res<UiFont>,
) {
    let ds = DisplayState::new_for(&session.run);
    queue.0.clear();
    queue.0.extend(pending.0.drain(..));
    spawn_combat_ui(&mut commands, &font, &ds);
    commands.insert_resource(ds);
}

fn exit_combat(
    mut commands: Commands,
    mut queue: ResMut<EventQueue>,
    mut pending: ResMut<PendingCard>,
    roots: Query<Entity, With<CombatRoot>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    queue.0.clear();
    pending.0 = None;
    commands.remove_resource::<DisplayState>();
}

fn spawn_combat_ui(commands: &mut Commands, font: &UiFont, ds: &DisplayState) {
    commands
        .spawn((
            CombatRoot,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| {
            // ---- top bar ----
            root.spawn(Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(12.)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|bar| {
                bar.spawn((Bind::Turn, theme::text(font, "", 20., theme::TEXT_DIM)));
                bar.spawn((Bind::Piles, theme::text(font, "", 20., theme::TEXT_DIM)));
            });

            // ---- battlefield ----
            root.spawn(Node {
                width: Val::Percent(100.),
                flex_grow: 1.,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(24.)),
                column_gap: Val::Px(40.),
                ..default()
            })
            .with_children(|field| {
                // player panel
                field
                    .spawn((
                        PanelTarget(TargetRef::Player),
                        Node {
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(16.)),
                            row_gap: Val::Px(6.),
                            min_width: Val::Px(220.),
                            ..default()
                        },
                        BackgroundColor(theme::PANEL),
                    ))
                    .with_children(|p| {
                        p.spawn(theme::text(font, "The Berserker", 24., theme::ACCENT));
                        p.spawn((
                            Bind::Hp(TargetRef::Player),
                            theme::text(font, "", 22., theme::HP_COLOR),
                        ));
                        p.spawn((
                            Bind::Block(TargetRef::Player),
                            theme::text(font, "", 18., theme::BLOCK_COLOR),
                        ));
                        p.spawn((
                            Bind::Status(TargetRef::Player),
                            theme::text(font, "", 16., theme::TEXT_DIM),
                        ));
                    });

                // enemies, in spawn order
                field
                    .spawn(Node {
                        flex_grow: 1.,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: Val::Px(24.),
                        ..default()
                    })
                    .with_children(|row| {
                        for (i, enemy) in ds.enemies.iter().enumerate() {
                            row.spawn((
                                PanelTarget(TargetRef::Enemy(i)),
                                Button, // clickable for targeting
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    padding: UiRect::all(Val::Px(16.)),
                                    row_gap: Val::Px(6.),
                                    min_width: Val::Px(200.),
                                    ..default()
                                },
                                BackgroundColor(theme::PANEL),
                            ))
                            .with_children(|p| {
                                p.spawn(theme::text(font, enemy.name, 22., theme::TEXT));
                                p.spawn((
                                    Bind::Intent(i),
                                    theme::text(font, "", 18., theme::ENERGY_COLOR),
                                ));
                                p.spawn((
                                    Bind::Hp(TargetRef::Enemy(i)),
                                    theme::text(font, "", 20., theme::HP_COLOR),
                                ));
                                p.spawn((
                                    Bind::Block(TargetRef::Enemy(i)),
                                    theme::text(font, "", 16., theme::BLOCK_COLOR),
                                ));
                                p.spawn((
                                    Bind::Status(TargetRef::Enemy(i)),
                                    theme::text(font, "", 14., theme::TEXT_DIM),
                                ));
                            });
                        }
                    });
            });

            // ---- bottom bar: energy, hand, end turn ----
            root.spawn(Node {
                width: Val::Percent(100.),
                padding: UiRect::all(Val::Px(12.)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.),
                ..default()
            })
            .with_children(|bar| {
                bar.spawn((
                    Bind::Energy,
                    theme::text(font, "", 30., theme::ENERGY_COLOR),
                ));
                bar.spawn((
                    HandRow,
                    Node {
                        flex_grow: 1.,
                        column_gap: Val::Px(10.),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ));
                theme::button(bar, font, EndTurnButton, "End Turn [E]");
            });
        });
}

fn status_line(s: &Statuses) -> String {
    let mut parts = Vec::new();
    if s.strength != 0 {
        parts.push(format!("Str {:+}", s.strength));
    }
    if s.vulnerable > 0 {
        parts.push(format!("Vuln {}", s.vulnerable));
    }
    if s.weak > 0 {
        parts.push(format!("Weak {}", s.weak));
    }
    if s.ritual > 0 {
        parts.push(format!("Ritual {}", s.ritual));
    }
    if s.enrage > 0 {
        parts.push(format!("Enrage {}", s.enrage));
    }
    if let Some(c) = s.curl_up {
        parts.push(format!("Curl Up {c}"));
    }
    if s.strength_down > 0 {
        parts.push(format!("Str Down {}", s.strength_down));
    }
    parts.join("  ")
}

fn intent_line(intent: Option<IntentKind>) -> String {
    match intent {
        Some(IntentKind::Attack { damage, hits: 1 }) => format!("Intent: ATK {damage}"),
        Some(IntentKind::Attack { damage, hits }) => format!("Intent: ATK {damage}x{hits}"),
        Some(IntentKind::AttackDefend { damage }) => format!("Intent: ATK {damage} +DEF"),
        Some(IntentKind::Defend) => "Intent: DEFEND".into(),
        Some(IntentKind::Buff) => "Intent: BUFF".into(),
        Some(IntentKind::Debuff) => "Intent: DEBUFF".into(),
        None => String::new(),
    }
}

fn sync_texts(ds: Res<DisplayState>, mut q: Query<(&Bind, &mut Text)>) {
    if !ds.is_changed() {
        return;
    }
    for (bind, mut text) in &mut q {
        text.0 = match bind {
            Bind::Turn => format!("Turn {}", ds.turn),
            Bind::Piles => format!("Draw {}   Discard {}", ds.draw_count, ds.discard_count),
            Bind::Energy => format!("Energy {}/3", ds.energy),
            Bind::Hp(TargetRef::Player) => format!("HP {}/{}", ds.player_hp, ds.player_max_hp),
            Bind::Hp(TargetRef::Enemy(i)) => match ds.enemies.get(*i) {
                Some(e) if e.alive => format!("HP {}/{}", e.hp, e.max_hp),
                _ => "DEAD".into(),
            },
            Bind::Block(TargetRef::Player) => block_line(ds.player_block),
            Bind::Block(TargetRef::Enemy(i)) => {
                block_line(ds.enemies.get(*i).map(|e| e.block).unwrap_or(0))
            }
            Bind::Status(TargetRef::Player) => status_line(&ds.statuses),
            Bind::Status(TargetRef::Enemy(i)) => ds
                .enemies
                .get(*i)
                .map(|e| status_line(&e.statuses))
                .unwrap_or_default(),
            Bind::Intent(i) => ds
                .enemies
                .get(*i)
                .map(|e| intent_line(e.intent))
                .unwrap_or_default(),
        };
    }
}

fn block_line(block: u32) -> String {
    if block > 0 {
        format!("Block {block}")
    } else {
        String::new()
    }
}

/// Highlight valid targets while a card is pending (and the keyboard cursor).
fn highlight_enemies(
    ds: Res<DisplayState>,
    pending: Res<PendingCard>,
    cursor: Res<TargetCursor>,
    mut panels: Query<(&PanelTarget, &mut BackgroundColor, Option<&Interaction>)>,
) {
    for (panel, mut bg, interaction) in &mut panels {
        let TargetRef::Enemy(i) = panel.0 else {
            continue;
        };
        let alive = ds.enemies.get(i).map(|e| e.alive).unwrap_or(false);
        let targeting = pending.0.is_some() && alive;
        let hovered = matches!(interaction, Some(Interaction::Hovered));
        let cursor_here = targeting && cursor.0 == i;
        *bg = BackgroundColor(if targeting && (hovered || cursor_here) {
            theme::PANEL_HOVER
        } else if targeting {
            theme::ACCENT.with_alpha(0.25)
        } else {
            theme::PANEL
        });
    }
}

fn dispatch(action: Action, session: &mut Session, queue: &mut EventQueue) {
    match session.run.apply(action) {
        Ok(events) => queue.0.extend(events),
        // The UI should have prevented this; the core stayed consistent.
        Err(err) => warn!("rejected action {action:?}: {err:?}"),
    }
}

/// Click (or hotkey) a card: dispatch immediately, or arm targeting mode.
fn try_play(
    index: usize,
    ds: &DisplayState,
    pending: &mut PendingCard,
    cursor: &mut TargetCursor,
    session: &mut Session,
    queue: &mut EventQueue,
) {
    let Some(card) = ds.hand.get(index) else {
        return;
    };
    let spec = card.spec();
    if spec.cost > ds.energy {
        return;
    }
    let living: Vec<usize> = ds
        .enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive)
        .map(|(i, _)| i)
        .collect();
    match spec.targeting {
        Targeting::SingleEnemy if living.len() > 1 => {
            pending.0 = Some(index);
            cursor.0 = living[0];
        }
        _ => dispatch(
            Action::PlayCard {
                hand_index: index,
                target: None,
            },
            session,
            queue,
        ),
    }
}

fn card_click(
    ds: Res<DisplayState>,
    mut pending: ResMut<PendingCard>,
    mut cursor: ResMut<TargetCursor>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    cards: Query<(&Interaction, &hand::Card), Changed<Interaction>>,
) {
    for (interaction, card) in &cards {
        if *interaction == Interaction::Pressed {
            try_play(card.slot, &ds, &mut pending, &mut cursor, &mut session, &mut queue);
        }
    }
}

fn enemy_click(
    ds: Res<DisplayState>,
    mut pending: ResMut<PendingCard>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    panels: Query<(&Interaction, &PanelTarget), Changed<Interaction>>,
) {
    let Some(card_index) = pending.0 else { return };
    for (interaction, panel) in &panels {
        let TargetRef::Enemy(i) = panel.0 else {
            continue;
        };
        let alive = ds.enemies.get(i).map(|e| e.alive).unwrap_or(false);
        if *interaction == Interaction::Pressed && alive {
            pending.0 = None;
            dispatch(
                Action::PlayCard {
                    hand_index: card_index,
                    target: Some(i),
                },
                &mut session,
                &mut queue,
            );
            return;
        }
    }
}

fn end_turn_button(
    mut pending: ResMut<PendingCard>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<EndTurnButton>)>,
) {
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            pending.0 = None;
            dispatch(Action::EndTurn, &mut session, &mut queue);
        }
    }
}

const DIGIT_KEYS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    ds: Res<DisplayState>,
    mut pending: ResMut<PendingCard>,
    mut cursor: ResMut<TargetCursor>,
    mut session: ResMut<Session>,
    mut queue: ResMut<EventQueue>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        pending.0 = None;
        return;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        pending.0 = None;
        dispatch(Action::EndTurn, &mut session, &mut queue);
        return;
    }
    if let Some(card_index) = pending.0 {
        // Targeting mode: cycle living enemies, Enter to confirm.
        let living: Vec<usize> = ds
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.alive)
            .map(|(i, _)| i)
            .collect();
        if living.is_empty() {
            pending.0 = None;
            return;
        }
        let pos = living.iter().position(|&i| i == cursor.0).unwrap_or(0);
        if keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::ArrowRight) {
            cursor.0 = living[(pos + 1) % living.len()];
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            cursor.0 = living[(pos + living.len() - 1) % living.len()];
        }
        if keys.just_pressed(KeyCode::Enter) {
            pending.0 = None;
            dispatch(
                Action::PlayCard {
                    hand_index: card_index,
                    target: Some(cursor.0),
                },
                &mut session,
                &mut queue,
            );
        }
        return;
    }
    for (n, key) in DIGIT_KEYS.iter().enumerate() {
        if keys.just_pressed(*key) {
            try_play(n, &ds, &mut pending, &mut cursor, &mut session, &mut queue);
        }
    }
}

/// When the fight's outcome has fully animated, follow the run's stage.
fn post_combat(
    ds: Res<DisplayState>,
    queue: Res<EventQueue>,
    session: Res<Session>,
    mut next: ResMut<NextState<AppState>>,
) {
    if ds.outcome.is_none() || !queue.0.is_empty() {
        return;
    }
    match session.run.stage {
        Stage::Reward { .. } => next.set(AppState::Reward),
        Stage::Victory => next.set(AppState::Victory),
        Stage::Defeat => next.set(AppState::GameOver),
        Stage::ChoosingNode => next.set(AppState::Map),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemePlugin;
    use crate::{CliSeed, Session};
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use helheim_core::run::RunState;

    #[test]
    fn card_assets_load_at_plugin_build() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));
        app.init_asset::<bevy::text::Font>();
        app.init_asset::<Image>();
        app.init_state::<AppState>();
        app.insert_resource(CliSeed(Some(0)));
        app.insert_resource(Session { run: RunState::new(0) });
        app.add_plugins((ThemePlugin, crate::anim::AnimPlugin, CombatScreenPlugin));
        app.update();
        assert!(app.world().contains_resource::<hand::CardAssets>());
    }
}
