use std::collections::VecDeque;

use bevy::prelude::*;
use helheim_core::cards::{CardId, CardKind};
use helheim_core::combat::{CombatEvent, IntentKind, Outcome, TargetRef};
use helheim_core::run::RunState;
use helheim_core::statuses::{StatusKind, Statuses};

use crate::theme::{self, UiFont};

pub const BEAT_SECONDS: f32 = 0.18;

#[derive(Clone, Debug, PartialEq)]
pub struct EnemyView {
    pub name: &'static str,
    pub hp: u32,
    pub max_hp: u32,
    pub block: u32,
    pub statuses: Statuses,
    pub intent: Option<IntentKind>,
    pub alive: bool,
}

/// What the player currently SEES. Converges to the core state as the
/// event queue drains; equal to it when the queue is empty.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct DisplayState {
    pub player_hp: u32,
    pub player_max_hp: u32,
    pub player_block: u32,
    pub energy: u32,
    pub statuses: Statuses,
    pub hand: Vec<CardId>,
    pub draw_count: u32,
    pub discard_count: u32,
    pub enemies: Vec<EnemyView>,
    pub outcome: Option<Outcome>,
    pub turn: u32,
}

impl DisplayState {
    /// Pre-replay snapshot of a just-begun fight: enemies at full HP, empty
    /// hand, no energy — the opening events animate everything in.
    pub fn new_for(run: &RunState) -> Self {
        let c = run.combat.as_ref().expect("fight begun");
        DisplayState {
            player_hp: c.player.hp,
            player_max_hp: c.player.max_hp,
            player_block: 0,
            energy: 0,
            statuses: Statuses::default(),
            hand: Vec::new(),
            draw_count: (c.draw.len() + c.hand.len()) as u32,
            discard_count: 0,
            enemies: c
                .enemies
                .iter()
                .map(|e| EnemyView {
                    name: e.species.name(),
                    hp: e.max_hp,
                    max_hp: e.max_hp,
                    block: 0,
                    statuses: Statuses {
                        curl_up: e.statuses.curl_up,
                        ..Default::default()
                    },
                    intent: None,
                    alive: true,
                })
                .collect(),
            outcome: None,
            turn: 0,
        }
    }
}

/// Map one core event onto the display. Pure data; unit-tested below.
pub fn apply_event(ds: &mut DisplayState, ev: &CombatEvent) {
    match *ev {
        CombatEvent::TurnStarted { turn } => ds.turn = turn,
        CombatEvent::EnergySet { energy } => ds.energy = energy,
        CombatEvent::CardDrawn { card } => {
            ds.draw_count = ds.draw_count.saturating_sub(1);
            ds.hand.push(card);
        }
        CombatEvent::DeckShuffled => {
            ds.draw_count += ds.discard_count;
            ds.discard_count = 0;
        }
        CombatEvent::CardPlayed { card, hand_index } => {
            if hand_index < ds.hand.len() {
                ds.hand.remove(hand_index);
            }
            if card.spec().kind != CardKind::Power {
                ds.discard_count += 1;
            }
        }
        CombatEvent::CardAddedToDiscard { .. } => ds.discard_count += 1,
        CombatEvent::HandDiscarded => {
            ds.discard_count += ds.hand.len() as u32;
            ds.hand.clear();
        }
        CombatEvent::BlockReset { target } => match target {
            TargetRef::Player => ds.player_block = 0,
            TargetRef::Enemy(i) => {
                if let Some(e) = ds.enemies.get_mut(i) {
                    e.block = 0;
                }
            }
        },
        CombatEvent::BlockGained { target, amount } => match target {
            TargetRef::Player => ds.player_block += amount,
            TargetRef::Enemy(i) => {
                if let Some(e) = ds.enemies.get_mut(i) {
                    e.block += amount;
                }
            }
        },
        CombatEvent::DamageDealt {
            target,
            blocked,
            hp_lost,
            ..
        } => match target {
            TargetRef::Player => {
                ds.player_block = ds.player_block.saturating_sub(blocked);
                ds.player_hp = ds.player_hp.saturating_sub(hp_lost);
            }
            TargetRef::Enemy(i) => {
                if let Some(e) = ds.enemies.get_mut(i) {
                    e.block = e.block.saturating_sub(blocked);
                    e.hp = e.hp.saturating_sub(hp_lost);
                }
            }
        },
        CombatEvent::StatusApplied {
            target,
            status,
            amount,
        } => {
            if let Some(s) = statuses_of(ds, target) {
                bump_status(s, status, amount);
            }
        }
        CombatEvent::StatusTicked {
            target,
            status,
            remaining,
        } => {
            if let Some(s) = statuses_of(ds, target) {
                set_duration(s, status, remaining);
            }
        }
        CombatEvent::StatusExpired { target, status } => {
            if let Some(s) = statuses_of(ds, target) {
                clear_status(s, status);
            }
        }
        CombatEvent::EnemyMoved { .. } => {}
        CombatEvent::IntentSet { index, intent } => {
            if let Some(e) = ds.enemies.get_mut(index) {
                e.intent = Some(intent);
            }
        }
        CombatEvent::EnemyDied { index } => {
            if let Some(e) = ds.enemies.get_mut(index) {
                e.alive = false;
                e.intent = None;
            }
        }
        CombatEvent::PlayerDied => ds.outcome = Some(Outcome::Defeat),
        CombatEvent::Victory => ds.outcome = Some(Outcome::Victory),
    }
}

fn statuses_of(ds: &mut DisplayState, target: TargetRef) -> Option<&mut Statuses> {
    match target {
        TargetRef::Player => Some(&mut ds.statuses),
        TargetRef::Enemy(i) => ds.enemies.get_mut(i).map(|e| &mut e.statuses),
    }
}

fn bump_status(s: &mut Statuses, kind: StatusKind, amount: i32) {
    match kind {
        StatusKind::Strength => s.strength += amount,
        StatusKind::Vulnerable => s.vulnerable += amount.max(0) as u32,
        StatusKind::Weak => s.weak += amount.max(0) as u32,
        StatusKind::Ritual => s.ritual += amount.max(0) as u32,
        StatusKind::Enrage => s.enrage += amount.max(0) as u32,
        StatusKind::CurlUp => s.curl_up = Some(amount.max(0) as u32),
        StatusKind::StrengthDown => s.strength_down += amount.max(0) as u32,
    }
}

fn set_duration(s: &mut Statuses, kind: StatusKind, remaining: u32) {
    match kind {
        StatusKind::Vulnerable => s.vulnerable = remaining,
        StatusKind::Weak => s.weak = remaining,
        _ => {}
    }
}

fn clear_status(s: &mut Statuses, kind: StatusKind) {
    match kind {
        StatusKind::Strength => s.strength = 0,
        StatusKind::Vulnerable => s.vulnerable = 0,
        StatusKind::Weak => s.weak = 0,
        StatusKind::Ritual => s.ritual = 0,
        StatusKind::Enrage => s.enrage = 0,
        StatusKind::CurlUp => s.curl_up = None,
        StatusKind::StrengthDown => s.strength_down = 0,
    }
}

// ---------- queue, beats, floaters ----------

#[derive(Resource, Default)]
pub struct EventQueue(pub VecDeque<CombatEvent>);

/// Events produced by `enter_node` that the combat screen must drain into the
/// `EventQueue` so the opening draw/intent animations play correctly.
#[derive(Resource, Default)]
pub struct PendingEvents(pub Vec<CombatEvent>);

#[derive(Resource)]
pub struct BeatTimer(pub Timer);

/// Marks a UI panel as the visual home of a combatant (floaters spawn here).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct PanelTarget(pub TargetRef);

#[derive(Component)]
struct Floater {
    timer: Timer,
}

/// run_if condition: player input is allowed only when nothing is animating.
pub fn queue_empty(queue: Res<EventQueue>) -> bool {
    queue.0.is_empty()
}

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventQueue>()
            .init_resource::<PendingEvents>()
            .insert_resource(BeatTimer(Timer::from_seconds(
                BEAT_SECONDS,
                TimerMode::Repeating,
            )))
            .add_systems(
                Update,
                (
                    drain_queue.run_if(resource_exists::<DisplayState>),
                    float_floaters,
                ),
            );
    }
}

/// Which events pause the replay for a visible beat, and what they show.
fn beat_visual(ev: &CombatEvent) -> Option<(TargetRef, String, Color)> {
    match ev {
        CombatEvent::DamageDealt {
            target, hp_lost, ..
        } => Some((
            *target,
            if *hp_lost > 0 {
                format!("-{hp_lost}")
            } else {
                "Blocked".into()
            },
            if *hp_lost > 0 {
                theme::HP_COLOR
            } else {
                theme::TEXT_DIM
            },
        )),
        CombatEvent::BlockGained { target, amount } => {
            Some((*target, format!("+{amount} Block"), theme::BLOCK_COLOR))
        }
        CombatEvent::EnemyMoved { index, mv } => {
            Some((TargetRef::Enemy(*index), mv.name().to_string(), theme::TEXT))
        }
        CombatEvent::StatusApplied {
            target,
            status,
            amount,
        } => Some((*target, format!("{status:?} {amount:+}"), theme::TEXT_DIM)),
        _ => None,
    }
}

fn is_beat(ev: &CombatEvent) -> bool {
    beat_visual(ev).is_some() || matches!(ev, CombatEvent::Victory | CombatEvent::PlayerDied)
}

/// Pop events each beat: bookkeeping applies instantly, beat events pause.
fn drain_queue(
    time: Res<Time>,
    mut timer: ResMut<BeatTimer>,
    mut queue: ResMut<EventQueue>,
    mut ds: ResMut<DisplayState>,
    mut commands: Commands,
    font: Res<UiFont>,
    panels: Query<(Entity, &PanelTarget)>,
) {
    timer.0.tick(time.delta());
    if queue.0.is_empty() || !timer.0.just_finished() {
        return;
    }
    while let Some(ev) = queue.0.pop_front() {
        let visual = beat_visual(&ev);
        let beat = is_beat(&ev);
        apply_event(&mut ds, &ev);
        if let Some((target, text, color)) = visual {
            if let Some((panel, _)) = panels.iter().find(|(_, p)| p.0 == target) {
                spawn_floater(&mut commands, &font, panel, text, color);
            }
        }
        if beat {
            break;
        }
    }
}

fn spawn_floater(
    commands: &mut Commands,
    font: &UiFont,
    parent: Entity,
    label: String,
    color: Color,
) {
    let floater = commands
        .spawn((
            Floater {
                timer: Timer::from_seconds(0.7, TimerMode::Once),
            },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(-12.),
                right: Val::Px(6.),
                ..default()
            },
            theme::text(font, label, 24., color),
            ZIndex(10),
        ))
        .id();
    commands.entity(parent).add_child(floater);
}

fn float_floaters(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Floater, &mut Node, &mut TextColor)>,
) {
    for (e, mut f, mut node, mut color) in &mut q {
        f.timer.tick(time.delta());
        let t = f.timer.fraction();
        node.top = Val::Px(-12. - 34. * t);
        color.0 = color.0.with_alpha(1.0 - t);
        if f.timer.is_finished() {
            commands.entity(e).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helheim_core::cards::CardId;
    use helheim_core::combat::{CombatEvent, IntentKind, Outcome, TargetRef};
    use helheim_core::statuses::StatusKind;

    fn fixture() -> DisplayState {
        DisplayState {
            player_hp: 80,
            player_max_hp: 80,
            player_block: 0,
            energy: 3,
            statuses: Default::default(),
            hand: vec![CardId::Hew, CardId::RaiseShield],
            draw_count: 5,
            discard_count: 3,
            enemies: vec![EnemyView {
                name: "Grave Wolf",
                hp: 40,
                max_hp: 44,
                block: 0,
                statuses: Default::default(),
                intent: None,
                alive: true,
            }],
            outcome: None,
            turn: 1,
        }
    }

    #[test]
    fn damage_event_updates_hp_and_block() {
        let mut ds = fixture();
        ds.enemies[0].block = 3;
        apply_event(
            &mut ds,
            &CombatEvent::DamageDealt {
                target: TargetRef::Enemy(0),
                amount: 8,
                blocked: 3,
                hp_lost: 5,
            },
        );
        assert_eq!(ds.enemies[0].hp, 35);
        assert_eq!(ds.enemies[0].block, 0);

        apply_event(
            &mut ds,
            &CombatEvent::DamageDealt {
                target: TargetRef::Player,
                amount: 6,
                blocked: 0,
                hp_lost: 6,
            },
        );
        assert_eq!(ds.player_hp, 74);
    }

    #[test]
    fn card_flow_events_track_zone_counts() {
        let mut ds = fixture();
        apply_event(
            &mut ds,
            &CombatEvent::CardPlayed {
                card: CardId::Hew,
                hand_index: 0,
            },
        );
        assert_eq!(ds.hand, vec![CardId::RaiseShield]);
        assert_eq!(ds.discard_count, 4);

        apply_event(
            &mut ds,
            &CombatEvent::CardDrawn {
                card: CardId::TwinAxes,
            },
        );
        assert_eq!(ds.hand, vec![CardId::RaiseShield, CardId::TwinAxes]);
        assert_eq!(ds.draw_count, 4);

        apply_event(&mut ds, &CombatEvent::HandDiscarded);
        assert!(ds.hand.is_empty());
        assert_eq!(ds.discard_count, 6);

        apply_event(&mut ds, &CombatEvent::DeckShuffled);
        assert_eq!(ds.draw_count, 10);
        assert_eq!(ds.discard_count, 0);
    }

    #[test]
    fn powers_do_not_join_the_discard_count() {
        let mut ds = fixture();
        ds.hand = vec![CardId::Berserkergang];
        apply_event(
            &mut ds,
            &CombatEvent::CardPlayed {
                card: CardId::Berserkergang,
                hand_index: 0,
            },
        );
        assert!(ds.hand.is_empty());
        assert_eq!(ds.discard_count, 3, "powers are consumed");
    }

    #[test]
    fn status_events_mutate_the_right_creature() {
        let mut ds = fixture();
        apply_event(
            &mut ds,
            &CombatEvent::StatusApplied {
                target: TargetRef::Enemy(0),
                status: StatusKind::Vulnerable,
                amount: 2,
            },
        );
        assert_eq!(ds.enemies[0].statuses.vulnerable, 2);
        apply_event(
            &mut ds,
            &CombatEvent::StatusTicked {
                target: TargetRef::Enemy(0),
                status: StatusKind::Vulnerable,
                remaining: 1,
            },
        );
        assert_eq!(ds.enemies[0].statuses.vulnerable, 1);
        apply_event(
            &mut ds,
            &CombatEvent::StatusExpired {
                target: TargetRef::Enemy(0),
                status: StatusKind::Vulnerable,
            },
        );
        assert_eq!(ds.enemies[0].statuses.vulnerable, 0);

        apply_event(
            &mut ds,
            &CombatEvent::StatusApplied {
                target: TargetRef::Player,
                status: StatusKind::Strength,
                amount: -2,
            },
        );
        assert_eq!(ds.statuses.strength, -2);
    }

    #[test]
    fn lifecycle_events_set_turn_energy_intent_outcome() {
        let mut ds = fixture();
        apply_event(&mut ds, &CombatEvent::TurnStarted { turn: 3 });
        apply_event(&mut ds, &CombatEvent::EnergySet { energy: 2 });
        apply_event(
            &mut ds,
            &CombatEvent::IntentSet {
                index: 0,
                intent: IntentKind::Attack {
                    damage: 11,
                    hits: 1,
                },
            },
        );
        assert_eq!(ds.turn, 3);
        assert_eq!(ds.energy, 2);
        assert_eq!(
            ds.enemies[0].intent,
            Some(IntentKind::Attack {
                damage: 11,
                hits: 1
            })
        );

        apply_event(&mut ds, &CombatEvent::EnemyDied { index: 0 });
        assert!(!ds.enemies[0].alive);
        assert_eq!(ds.enemies[0].intent, None);

        apply_event(&mut ds, &CombatEvent::Victory);
        assert_eq!(ds.outcome, Some(Outcome::Victory));
    }

    #[test]
    fn block_events_gain_and_reset() {
        let mut ds = fixture();
        apply_event(
            &mut ds,
            &CombatEvent::BlockGained {
                target: TargetRef::Player,
                amount: 5,
            },
        );
        assert_eq!(ds.player_block, 5);
        apply_event(
            &mut ds,
            &CombatEvent::BlockReset {
                target: TargetRef::Player,
            },
        );
        assert_eq!(ds.player_block, 0);
    }
}
