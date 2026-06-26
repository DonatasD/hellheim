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

/// Rebuild the whole hand on display change (replaced by reconcile in a later task).
pub fn rebuild_hand(
    mut commands: Commands,
    ds: Res<DisplayState>,
    font: Res<UiFont>,
    assets: Res<CardAssets>,
    row: Query<Entity, With<HandRow>>,
    existing: Query<Entity, With<Card>>,
) {
    if !ds.is_changed() {
        return;
    }
    let Ok(row) = row.single() else { return };
    for e in &existing {
        commands.entity(e).despawn();
    }
    for (i, card) in ds.hand.iter().enumerate() {
        let e = spawn_card(&mut commands, &font, &assets, *card, i, ds.energy);
        commands.entity(row).add_child(e);
    }
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
