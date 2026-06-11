use crate::cards::CardId;
use crate::enemies::{roll_move, EnemyMove, Species};
use crate::rng::RunRng;
use crate::statuses::{StatusKind, Statuses};

pub const ENERGY_PER_TURN: u32 = 3;
pub const DRAW_PER_TURN: usize = 5;
pub const HAND_LIMIT: usize = 10;

#[derive(Clone, Debug)]
pub struct Player {
    pub hp: u32,
    pub max_hp: u32,
    pub block: u32,
    pub energy: u32,
    pub statuses: Statuses,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub species: Species,
    pub hp: u32,
    pub max_hp: u32,
    pub block: u32,
    pub statuses: Statuses,
    /// Rolled 5–7 at spawn for rats; 0 for species without a Bite.
    pub bite_damage: u32,
    pub next_move: EnemyMove,
    pub history: Vec<EnemyMove>,
}

impl Enemy {
    pub fn alive(&self) -> bool {
        self.hp > 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome { Victory, Defeat }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayCard { hand_index: usize, target: Option<usize> },
    EndTurn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IllegalAction { CombatOver, NoSuchCard, NotEnoughEnergy, NeedsTarget, InvalidTarget }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetRef { Player, Enemy(usize) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntentKind {
    Attack { damage: u32, hits: u32 },
    AttackDefend { damage: u32 },
    Defend,
    Buff,
    Debuff,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CombatEvent {
    TurnStarted { turn: u32 },
    EnergySet { energy: u32 },
    CardDrawn { card: CardId },
    DeckShuffled,
    CardPlayed { card: CardId, hand_index: usize },
    CardAddedToDiscard { card: CardId },
    HandDiscarded,
    BlockReset { target: TargetRef },
    BlockGained { target: TargetRef, amount: u32 },
    DamageDealt { target: TargetRef, amount: u32, blocked: u32, hp_lost: u32 },
    StatusApplied { target: TargetRef, status: StatusKind, amount: i32 },
    StatusTicked { target: TargetRef, status: StatusKind, remaining: u32 },
    StatusExpired { target: TargetRef, status: StatusKind },
    EnemyMoved { index: usize, mv: EnemyMove },
    IntentSet { index: usize, intent: IntentKind },
    EnemyDied { index: usize },
    PlayerDied,
    Victory,
}

#[derive(Clone, Debug)]
pub struct CombatState {
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub draw: Vec<CardId>,
    pub hand: Vec<CardId>,
    pub discard: Vec<CardId>,
    pub exhaust: Vec<CardId>,
    pub turn: u32,
    pub over: Option<Outcome>,
}

impl CombatState {
    /// Begin a fight. RNG draw order per enemy: HP, then bite (rats), then
    /// Curl Up (rats) — keep this order stable, determinism tests depend on it.
    pub fn new(
        rng: &mut RunRng,
        deck: &[CardId],
        hp: u32,
        max_hp: u32,
        species: &[Species],
    ) -> (CombatState, Vec<CombatEvent>) {
        let mut events = Vec::new();
        let mut draw = deck.to_vec();
        rng.shuffle(&mut draw);

        let enemies: Vec<Enemy> = species
            .iter()
            .map(|&sp| {
                let (lo, hi) = sp.hp_range();
                let hp = rng.range(lo, hi);
                let is_rat = matches!(sp, Species::BarrowRat | Species::FenRat);
                let bite_damage = if is_rat { rng.range(5, 7) } else { 0 };
                let curl_up = is_rat.then(|| rng.range(3, 7));
                Enemy {
                    species: sp,
                    hp,
                    max_hp: hp,
                    block: 0,
                    statuses: Statuses { curl_up, ..Default::default() },
                    bite_damage,
                    next_move: EnemyMove::Chant, // placeholder, rolled below
                    history: Vec::new(),
                }
            })
            .collect();

        let mut state = CombatState {
            player: Player { hp, max_hp, block: 0, energy: ENERGY_PER_TURN, statuses: Statuses::default() },
            enemies,
            draw,
            hand: Vec::new(),
            discard: Vec::new(),
            exhaust: Vec::new(),
            turn: 1,
            over: None,
        };

        for i in 0..state.enemies.len() {
            state.enemies[i].next_move =
                roll_move(state.enemies[i].species, &state.enemies[i].history, rng);
            events.push(CombatEvent::IntentSet { index: i, intent: state.intent_of(i) });
        }

        events.push(CombatEvent::TurnStarted { turn: 1 });
        events.push(CombatEvent::EnergySet { energy: ENERGY_PER_TURN });
        for _ in 0..DRAW_PER_TURN {
            state.draw_one(rng, &mut events);
        }
        (state, events)
    }

    /// What the enemy's next move will do, with current modifiers baked in.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds for `enemies`.
    pub fn intent_of(&self, index: usize) -> IntentKind {
        let e = &self.enemies[index];
        let atk = |base: u32| {
            attack_damage(base, e.statuses.strength, e.statuses.weak > 0,
                          self.player.statuses.vulnerable > 0)
        };
        match e.next_move {
            EnemyMove::DarkStrike => IntentKind::Attack { damage: atk(6), hits: 1 },
            EnemyMove::Chomp => IntentKind::Attack { damage: atk(11), hits: 1 },
            EnemyMove::Rush => IntentKind::Attack { damage: atk(14), hits: 1 },
            EnemyMove::SkullBash => IntentKind::Attack { damage: atk(6), hits: 1 },
            EnemyMove::Bite => IntentKind::Attack { damage: atk(e.bite_damage), hits: 1 },
            EnemyMove::Thrash => IntentKind::AttackDefend { damage: atk(7) },
            EnemyMove::Chant | EnemyMove::Bellow | EnemyMove::Grow | EnemyMove::TrollBellow => {
                IntentKind::Buff
            }
            EnemyMove::Spittle => IntentKind::Debuff,
        }
    }

    /// Draw one card. Reshuffles discard into draw when needed; forfeits the
    /// draw silently at the hand limit or when both piles are empty.
    fn draw_one(&mut self, rng: &mut RunRng, events: &mut Vec<CombatEvent>) {
        if self.hand.len() >= HAND_LIMIT {
            return;
        }
        if self.draw.is_empty() {
            if self.discard.is_empty() {
                return;
            }
            self.draw.append(&mut self.discard);
            rng.shuffle(&mut self.draw);
            events.push(CombatEvent::DeckShuffled);
        }
        let card = self.draw.pop().expect("draw pile refilled above");
        self.hand.push(card);
        events.push(CombatEvent::CardDrawn { card });
    }

    pub fn apply(
        &mut self,
        rng: &mut RunRng,
        action: Action,
    ) -> Result<Vec<CombatEvent>, IllegalAction> {
        if self.over.is_some() {
            return Err(IllegalAction::CombatOver);
        }
        match action {
            Action::PlayCard { hand_index, target } => self.play_card(rng, hand_index, target),
            Action::EndTurn => Ok(self.end_turn(rng)),
        }
    }

    fn living(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.enemies.len()).filter(|&i| self.enemies[i].alive())
    }

    fn play_card(
        &mut self,
        rng: &mut RunRng,
        hand_index: usize,
        target: Option<usize>,
    ) -> Result<Vec<CombatEvent>, IllegalAction> {
        use crate::cards::{CardKind, Targeting};

        let card = *self.hand.get(hand_index).ok_or(IllegalAction::NoSuchCard)?;
        let spec = card.spec();
        if self.player.energy < spec.cost {
            return Err(IllegalAction::NotEnoughEnergy);
        }
        let target_idx = match spec.targeting {
            Targeting::SingleEnemy => match target {
                Some(t) => {
                    if self.enemies.get(t).map(Enemy::alive).unwrap_or(false) {
                        Some(t)
                    } else {
                        return Err(IllegalAction::InvalidTarget);
                    }
                }
                Option::None => {
                    let living: Vec<usize> = self.living().collect();
                    if living.len() == 1 {
                        Some(living[0]) // auto-target the only enemy
                    } else {
                        return Err(IllegalAction::NeedsTarget);
                    }
                }
            },
            Targeting::AllEnemies | Targeting::None => Option::None,
        };

        let mut events = Vec::new();
        self.hand.remove(hand_index);
        self.player.energy -= spec.cost;
        events.push(CombatEvent::CardPlayed { card, hand_index });
        events.push(CombatEvent::EnergySet { energy: self.player.energy });

        for effect in spec.effects {
            if self.over.is_some() {
                break; // victory stops remaining effects
            }
            self.run_effect(rng, *effect, card, target_idx, &mut events);
        }

        if spec.kind == CardKind::Skill && self.over.is_none() {
            self.trigger_enrage(&mut events);
        }
        if spec.kind != CardKind::Power {
            self.discard.push(card); // powers are consumed
        }
        Ok(events)
    }

    /// Player attacks one enemy: pipeline, soak, Curl Up, death, victory.
    fn attack_enemy(&mut self, i: usize, base: u32, events: &mut Vec<CombatEvent>) {
        let dmg = attack_damage(
            base,
            self.player.statuses.strength,
            self.player.statuses.weak > 0,
            self.enemies[i].statuses.vulnerable > 0,
        );
        let e = &mut self.enemies[i];
        let out = soak(&mut e.block, &mut e.hp, dmg);
        events.push(CombatEvent::DamageDealt {
            target: TargetRef::Enemy(i),
            amount: dmg,
            blocked: out.blocked,
            hp_lost: out.hp_lost,
        });
        if dmg >= 1 && e.hp > 0 {
            if let Some(curl) = e.statuses.curl_up.take() {
                e.block += curl;
                events.push(CombatEvent::BlockGained { target: TargetRef::Enemy(i), amount: curl });
                events.push(CombatEvent::StatusExpired { target: TargetRef::Enemy(i), status: StatusKind::CurlUp });
            }
        }
        if e.hp == 0 {
            events.push(CombatEvent::EnemyDied { index: i });
            if self.living().next().is_none() {
                self.over = Some(Outcome::Victory);
                events.push(CombatEvent::Victory);
            }
        }
    }

    fn trigger_enrage(&mut self, events: &mut Vec<CombatEvent>) {
        for i in 0..self.enemies.len() {
            let enrage = self.enemies[i].statuses.enrage;
            if self.enemies[i].alive() && enrage > 0 {
                self.enemies[i].statuses.strength += enrage as i32;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: enrage as i32,
                });
                events.push(CombatEvent::IntentSet { index: i, intent: self.intent_of(i) });
            }
        }
    }

    fn run_effect(
        &mut self,
        rng: &mut RunRng,
        effect: crate::cards::Effect,
        card: CardId,
        target: Option<usize>,
        events: &mut Vec<CombatEvent>,
    ) {
        use crate::cards::Effect;
        let _ = &rng;
        let _ = &card;
        match effect {
            Effect::Damage(base) => {
                if let Some(t) = target {
                    if self.enemies[t].alive() {
                        self.attack_enemy(t, base, events);
                    } // dead target: the hit fizzles, no event
                }
            }
            Effect::Block(n) => {
                self.player.block += n;
                events.push(CombatEvent::BlockGained { target: TargetRef::Player, amount: n });
            }
            _ => todo!("Tasks 11–12"),
        }
    }

    fn end_turn(&mut self, _rng: &mut RunRng) -> Vec<CombatEvent> {
        todo!("Task 13")
    }
}

/// StS damage pipeline, integer math (truncating division == floor here):
/// strength adds to base, Weak multiplies ×0.75, Vulnerable ×1.5.
pub fn attack_damage(base: u32, strength: i32, attacker_weak: bool, target_vulnerable: bool) -> u32 {
    let mut d = (base as i32 + strength).max(0) as u32;
    if attacker_weak {
        d = d * 3 / 4;
    }
    if target_vulnerable {
        d = d * 3 / 2;
    }
    d
}

#[must_use]
pub struct DamageOutcome {
    pub blocked: u32,
    pub hp_lost: u32,
}

/// Apply damage to block first, overflow to HP. Mutates in place.
pub fn soak(block: &mut u32, hp: &mut u32, amount: u32) -> DamageOutcome {
    let blocked = amount.min(*block);
    *block -= blocked;
    let hp_lost = (amount - blocked).min(*hp);
    *hp -= hp_lost;
    DamageOutcome { blocked, hp_lost }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{starter_deck, CardId};
    use crate::enemies::{EnemyMove, Species};
    use crate::rng::RunRng;
    use crate::statuses::Statuses;

    // ---- hand-built fixtures (no RNG dependence) ----

    pub fn enemy(species: Species, hp: u32) -> Enemy {
        Enemy {
            species,
            hp,
            max_hp: hp,
            block: 0,
            statuses: Statuses::default(),
            bite_damage: 6,
            next_move: EnemyMove::DarkStrike,
            history: vec![EnemyMove::Chant], // non-empty: not "turn 1" for AI rolls
        }
    }

    pub fn combat_vs(enemies: Vec<Enemy>, hand: Vec<CardId>) -> CombatState {
        CombatState {
            player: Player { hp: 80, max_hp: 80, block: 0, energy: 3, statuses: Statuses::default() },
            enemies,
            draw: vec![],
            hand,
            discard: vec![],
            exhaust: vec![],
            turn: 1,
            over: None,
        }
    }

    fn count_drawn(events: &[CombatEvent]) -> usize {
        events.iter().filter(|e| matches!(e, CombatEvent::CardDrawn { .. })).count()
    }

    #[test]
    fn new_combat_opens_with_5_cards_3_energy_and_intents() {
        let mut rng = RunRng::new(11);
        let (c, events) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        assert_eq!(c.hand.len(), 5);
        assert_eq!(c.draw.len(), 5);
        assert_eq!(c.player.energy, 3);
        assert_eq!(c.turn, 1);
        assert!(c.over.is_none());
        assert_eq!(count_drawn(&events), 5);
        assert!(events.iter().any(|e| matches!(e, CombatEvent::TurnStarted { turn: 1 })));
        assert!(events.iter().any(|e| matches!(e, CombatEvent::IntentSet { index: 0, .. })));
        // Wolf always opens with Chomp: base 11.
        assert_eq!(c.enemies[0].next_move, EnemyMove::Chomp);
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 11, hits: 1 }));
    }

    #[test]
    fn new_combat_rolls_hp_in_species_range() {
        let mut rng = RunRng::new(5);
        let (c, _) = CombatState::new(
            &mut rng, &starter_deck(), 80, 80,
            &[Species::BarrowRat, Species::FenRat],
        );
        let (lo0, hi0) = Species::BarrowRat.hp_range();
        assert!((lo0..=hi0).contains(&c.enemies[0].hp));
        // Rats spawn with Curl Up 3–7 and bite 5–7.
        for e in &c.enemies {
            let curl = e.statuses.curl_up.expect("rats spawn with Curl Up");
            assert!((3..=7).contains(&curl));
            assert!((5..=7).contains(&e.bite_damage));
        }
    }

    #[test]
    fn new_combat_is_deterministic_per_seed() {
        let mk = || {
            let mut rng = RunRng::new(77);
            CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::DraugrChanter])
        };
        let (a, ea) = mk();
        let (b, eb) = mk();
        assert_eq!(a.hand, b.hand);
        assert_eq!(a.enemies[0].hp, b.enemies[0].hp);
        assert_eq!(ea, eb);
    }

    #[test]
    fn intent_reflects_enemy_strength_and_player_vulnerable() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 6, hits: 1 }));
        c.enemies[0].statuses.strength = 3;
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 9, hits: 1 }));
        c.player.statuses.vulnerable = 1;
        assert!(matches!(c.intent_of(0), IntentKind::Attack { damage: 13, hits: 1 })); // floor(9*1.5)
    }

    fn play(c: &mut CombatState, i: usize, t: Option<usize>) -> Result<Vec<CombatEvent>, IllegalAction> {
        let mut rng = RunRng::new(0);
        c.apply(&mut rng, Action::PlayCard { hand_index: i, target: t })
    }

    #[test]
    fn hew_deals_6_costs_1_and_goes_to_discard() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::Hew]);
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 44);
        assert_eq!(c.player.energy, 2);
        assert!(c.hand.is_empty());
        assert_eq!(c.discard, vec![CardId::Hew]);
        assert!(events.contains(&CombatEvent::CardPlayed { card: CardId::Hew, hand_index: 0 }));
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0), amount: 6, blocked: 0, hp_lost: 6
        }));
        assert!(events.contains(&CombatEvent::EnergySet { energy: 2 }));
    }

    #[test]
    fn raise_shield_gains_5_block() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::RaiseShield]);
        let events = play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.block, 5);
        assert!(events.contains(&CombatEvent::BlockGained { target: TargetRef::Player, amount: 5 }));
    }

    #[test]
    fn attack_consumes_enemy_block_first() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::Hew]);
        c.enemies[0].block = 4;
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 48);
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0), amount: 6, blocked: 4, hp_lost: 2
        }));
    }

    #[test]
    fn not_enough_energy_is_rejected() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::SkullSplitter]);
        c.player.energy = 1; // Skull-Splitter costs 2
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::NotEnoughEnergy));
        assert_eq!(c.hand.len(), 1); // nothing changed
        assert_eq!(c.player.energy, 1);
    }

    #[test]
    fn bad_hand_index_is_rejected() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::NoSuchCard));
    }

    #[test]
    fn single_target_card_with_two_living_enemies_needs_a_target() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 12), enemy(Species::FenRat, 12)],
            vec![CardId::Hew],
        );
        assert_eq!(play(&mut c, 0, None), Err(IllegalAction::NeedsTarget));
    }

    #[test]
    fn single_living_enemy_is_auto_targeted() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 12), enemy(Species::FenRat, 12)],
            vec![CardId::Hew],
        );
        c.enemies[0].hp = 0; // only the Fen Rat lives
        play(&mut c, 0, None).unwrap();
        assert_eq!(c.enemies[1].hp, 6);
    }

    #[test]
    fn dead_or_out_of_range_target_is_rejected() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 12), enemy(Species::FenRat, 12)],
            vec![CardId::Hew, CardId::Hew],
        );
        c.enemies[0].hp = 0;
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::InvalidTarget));
        assert_eq!(play(&mut c, 0, Some(9)), Err(IllegalAction::InvalidTarget));
    }

    #[test]
    fn base_damage_passes_through() {
        assert_eq!(attack_damage(6, 0, false, false), 6);
    }

    #[test]
    fn strength_adds_before_multipliers() {
        assert_eq!(attack_damage(6, 3, false, false), 9);
    }

    #[test]
    fn negative_strength_clamps_at_zero() {
        assert_eq!(attack_damage(2, -5, false, false), 0);
    }

    #[test]
    fn weak_is_three_quarters_floored() {
        assert_eq!(attack_damage(6, 0, true, false), 4); // floor(4.5)
        assert_eq!(attack_damage(7, 0, true, false), 5); // floor(5.25)
        assert_eq!(attack_damage(8, 0, true, false), 6); // exact
    }

    #[test]
    fn vulnerable_is_one_and_a_half_floored() {
        assert_eq!(attack_damage(6, 0, false, true), 9); // exact
        assert_eq!(attack_damage(5, 0, false, true), 7); // floor(7.5)
    }

    #[test]
    fn pipeline_order_is_strength_then_weak_then_vulnerable() {
        // Distinguishing case: base 10 → weak 7 → vuln 10.
        // Vuln first would give: 10→15→11. Assert 10.
        assert_eq!(attack_damage(10, 0, true, true), 10);
    }

    #[test]
    fn soak_consumes_block_before_hp() {
        let mut block = 5;
        let mut hp = 80;
        let out = soak(&mut block, &mut hp, 8);
        assert_eq!((block, hp), (0, 77));
        assert_eq!((out.blocked, out.hp_lost), (5, 3));
    }

    #[test]
    fn soak_leaves_leftover_block() {
        let mut block = 10;
        let mut hp = 80;
        let out = soak(&mut block, &mut hp, 4);
        assert_eq!((block, hp), (6, 80));
        assert_eq!((out.blocked, out.hp_lost), (4, 0));
    }

    #[test]
    fn soak_does_not_underflow_hp() {
        let mut block = 0;
        let mut hp = 3;
        let out = soak(&mut block, &mut hp, 99);
        assert_eq!(hp, 0);
        assert_eq!(out.hp_lost, 3);
    }
}
