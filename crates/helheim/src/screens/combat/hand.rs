//! Combat hand: card visuals, the event-sourced reconcile, and animations.
use bevy::prelude::*;
use helheim_core::cards::{CardId, CardKind};

use super::HandRow;
use crate::anim::DisplayState;
use crate::theme::{self, UiFont};

/// Per-type accent: Attack red, Skill blue, Power gold.
pub fn kind_color(kind: CardKind) -> Color {
    match kind {
        CardKind::Attack => theme::ACCENT,
        CardKind::Skill => theme::BLOCK_COLOR,
        CardKind::Power => theme::ENERGY_COLOR,
    }
}

#[derive(Component)]
pub struct Card {
    pub slot: usize,
    pub card: CardId,
}

/// Full-card dark overlay; alpha rises when the card is unaffordable.
#[derive(Component)]
pub struct CardScrim;

/// Draw-in animation in progress.
#[derive(Component)]
pub struct CardEnter {
    pub timer: Timer,
}

/// Fly-to-discard animation in progress (then despawn).
#[derive(Component)]
pub struct CardFlyOut {
    pub timer: Timer,
}

pub const ENTER_SECS: f32 = 0.32;
pub const FLYOUT_SECS: f32 = 0.40;

const CARD_W: f32 = 138.0;
const CARD_H: f32 = 178.0;

/// Build one Option-C card entity (type frame + icon + watermark + cost gem) and
/// return it. Caller parents it into the hand row.
pub fn spawn_card(
    commands: &mut Commands,
    font: &UiFont,
    assets: &CardAssets,
    card: CardId,
    slot: usize,
    energy: u32,
) -> Entity {
    let spec = card.spec();
    let col = kind_color(spec.kind);
    let icon = assets.for_kind(spec.kind);
    let unaffordable = spec.cost > energy;
    let hot = if slot < 9 { format!("[{}]", slot + 1) } else { "[0]".into() };

    commands
        .spawn((
            Card { slot, card },
            Button,
            UiTransform::default(),
            Node {
                width: Val::Px(CARD_W),
                height: Val::Px(CARD_H),
                border: UiRect::all(Val::Px(2.5)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(9.)),
                border_radius: BorderRadius::all(Val::Px(10.)),
                ..default()
            },
            BorderColor::all(col),
            BackgroundColor(theme::PANEL),
        ))
        .with_children(|c| {
            // faint watermark icon
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(14.),
                    top: Val::Px(40.),
                    width: Val::Px(108.),
                    height: Val::Px(108.),
                    ..default()
                },
                ImageNode { color: col.with_alpha(0.10), ..ImageNode::new(icon.clone()) },
            ));
            // cost gem
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(6.),
                    top: Val::Px(6.),
                    width: Val::Px(24.),
                    height: Val::Px(24.),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(col),
            ))
            .with_children(|g| {
                g.spawn(theme::text(font, format!("{}", spec.cost), 14., Color::srgb(0.06, 0.06, 0.08)));
            });
            // big type icon
            c.spawn((
                Node { width: Val::Px(50.), height: Val::Px(50.), margin: UiRect::top(Val::Px(16.)), ..default() },
                ImageNode { color: col, ..ImageNode::new(icon) },
            ));
            // name / text / hotkey
            c.spawn(theme::text(font, spec.name, 14., theme::TEXT));
            c.spawn(theme::text(font, spec.text, 11.5, theme::TEXT_DIM));
            c.spawn(theme::text(font, hot, 11., theme::TEXT_DIM));
            // affordability scrim (covers the whole card)
            c.spawn((
                CardScrim,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.),
                    top: Val::Px(0.),
                    width: Val::Px(CARD_W),
                    height: Val::Px(CARD_H),
                    border_radius: BorderRadius::all(Val::Px(10.)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.04, 0.04, 0.06).with_alpha(if unaffordable { 0.5 } else { 0.0 })),
            ));
        })
        .id()
}

/// Reconcile card entities against the event stream: spawn drawn cards (with a
/// draw-in animation), fly out played/discarded cards, and keep slots in sync.
#[allow(clippy::too_many_arguments)]
pub fn reconcile_hand(
    mut commands: Commands,
    ds: Res<DisplayState>,
    font: Res<UiFont>,
    assets: Res<CardAssets>,
    row: Query<Entity, With<HandRow>>,
    mut flow: MessageReader<crate::anim::CardFlow>,
    mut cards: Query<(Entity, &mut Card), Without<CardFlyOut>>,
) {
    let Ok(row) = row.single() else { return };
    for f in flow.read() {
        match *f {
            crate::anim::CardFlow::Drawn(card) => {
                let slot = cards.iter().count();
                let e = spawn_card(&mut commands, &font, &assets, card, slot, ds.energy);
                commands.entity(e).insert(CardEnter { timer: Timer::from_seconds(ENTER_SECS, TimerMode::Once) });
                commands.entity(row).add_child(e);
            }
            crate::anim::CardFlow::Played { slot } => {
                for (e, mut card) in &mut cards {
                    if card.slot == slot {
                        commands.entity(e).remove::<Button>().insert(CardFlyOut {
                            timer: Timer::from_seconds(FLYOUT_SECS, TimerMode::Once),
                        });
                    } else if card.slot > slot {
                        card.slot -= 1;
                    }
                }
            }
            crate::anim::CardFlow::Discarded => {
                for (e, _) in &mut cards {
                    commands.entity(e).remove::<Button>().insert(CardFlyOut {
                        timer: Timer::from_seconds(FLYOUT_SECS, TimerMode::Once),
                    });
                }
            }
        }
    }
}

/// Darken cards the player can't currently afford (scrim alpha), on energy change.
pub fn refresh_affordability(
    ds: Res<DisplayState>,
    cards: Query<(&Card, &Children)>,
    mut scrims: Query<&mut BackgroundColor, With<CardScrim>>,
) {
    if !ds.is_changed() {
        return;
    }
    for (card, children) in &cards {
        let unaffordable = card.card.spec().cost > ds.energy;
        for child in children.iter() {
            if let Ok(mut bg) = scrims.get_mut(child) {
                bg.0 = bg.0.with_alpha(if unaffordable { 0.5 } else { 0.0 });
            }
        }
    }
}

/// Fly a played/discarded card to the discard pile (lower-right), then despawn.
pub fn animate_flyout(
    time: Res<Time>,
    mut commands: Commands,
    mut cards: Query<(Entity, &mut CardFlyOut, &mut Node, &mut UiTransform)>,
) {
    for (e, mut out, mut node, mut tf) in &mut cards {
        node.position_type = PositionType::Absolute; // pop out of the row so the rest reflow
        out.timer.tick(time.delta());
        let t = out.timer.fraction();
        tf.translation = Val2::px(320.0 * t, 130.0 * t);
        tf.scale = Vec2::splat(1.0 - 0.65 * t);
        if out.timer.is_finished() {
            commands.entity(e).despawn();
        }
    }
}

/// Slide a freshly drawn card in from the draw pile (lower-left) to rest.
pub fn animate_enter(
    time: Res<Time>,
    mut commands: Commands,
    mut cards: Query<(Entity, &mut CardEnter, &mut UiTransform)>,
) {
    for (e, mut enter, mut tf) in &mut cards {
        enter.timer.tick(time.delta());
        let t = enter.timer.fraction();
        let ease = t * t * (3.0 - 2.0 * t); // smoothstep
        tf.translation = Val2::px(-280.0 * (1.0 - ease), 120.0 * (1.0 - ease));
        tf.scale = Vec2::splat(0.5 + 0.5 * ease);
        if enter.timer.is_finished() {
            tf.translation = Val2::px(0.0, 0.0);
            tf.scale = Vec2::ONE;
            commands.entity(e).remove::<CardEnter>();
        }
    }
}

/// Move `current` a frame-rate-scaled fraction toward `target` (cap at 1.0).
pub fn approach(current: f32, target: f32, rate_dt: f32) -> f32 {
    current + (target - current) * rate_dt.min(1.0)
}

const HOVER_SCALE: f32 = 1.09;
const HOVER_LIFT: f32 = -14.0;

/// Hovered settled card eases up + scales; others settle back to identity.
pub fn hover_cards(
    time: Res<Time>,
    mut cards: Query<(&Interaction, &mut UiTransform), (With<Card>, Without<CardEnter>, Without<CardFlyOut>)>,
) {
    let dt = time.delta_secs() * 14.0;
    for (interaction, mut tf) in &mut cards {
        let hot = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let ts = if hot { HOVER_SCALE } else { 1.0 };
        let ty = if hot { HOVER_LIFT } else { 0.0 };
        let s = approach(tf.scale.x, ts, dt);
        tf.scale = Vec2::splat(s);
        let y = approach(px_y(&tf.translation), ty, dt);
        tf.translation = Val2::px(0.0, y);
    }
}

/// Read the px component of a `Val2`'s y (0 if not a `Px`).
fn px_y(t: &Val2) -> f32 {
    if let Val::Px(p) = t.y { p } else { 0.0 }
}

/// Card-type icon textures, loaded once and tinted per kind at spawn.
#[derive(Resource)]
pub struct CardAssets {
    attack: Handle<Image>,
    skill: Handle<Image>,
    power: Handle<Image>,
}

impl CardAssets {
    pub fn load(server: &AssetServer) -> Self {
        CardAssets {
            attack: server.load("icons/card_attack.png"),
            skill: server.load("icons/card_skill.png"),
            power: server.load("icons/card_power.png"),
        }
    }
    pub fn for_kind(&self, kind: CardKind) -> Handle<Image> {
        match kind {
            CardKind::Attack => self.attack.clone(),
            CardKind::Skill => self.skill.clone(),
            CardKind::Power => self.power.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_color_is_distinct_per_type() {
        assert_eq!(kind_color(CardKind::Attack), theme::ACCENT);
        assert_ne!(kind_color(CardKind::Attack), kind_color(CardKind::Skill));
        assert_ne!(kind_color(CardKind::Skill), kind_color(CardKind::Power));
    }
}

#[cfg(test)]
mod motion_tests {
    use super::approach;
    #[test]
    fn approach_moves_toward_and_clamps() {
        assert!((approach(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        assert!((approach(0.0, 1.0, 5.0) - 1.0).abs() < 1e-6); // clamped
    }
}
