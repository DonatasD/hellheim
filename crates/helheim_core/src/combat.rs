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
pub enum Outcome {
    Victory,
    Defeat,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    PlayCard {
        hand_index: usize,
        target: Option<usize>,
    },
    EndTurn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IllegalAction {
    CombatOver,
    NoSuchCard,
    NotEnoughEnergy,
    NeedsTarget,
    InvalidTarget,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetRef {
    Player,
    Enemy(usize),
}

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
    TurnStarted {
        turn: u32,
    },
    EnergySet {
        energy: u32,
    },
    CardDrawn {
        card: CardId,
    },
    DeckShuffled,
    CardPlayed {
        card: CardId,
        hand_index: usize,
    },
    CardAddedToDiscard {
        card: CardId,
    },
    HandDiscarded,
    BlockReset {
        target: TargetRef,
    },
    BlockGained {
        target: TargetRef,
        amount: u32,
    },
    DamageDealt {
        target: TargetRef,
        amount: u32,
        blocked: u32,
        hp_lost: u32,
    },
    Healed {
        target: TargetRef,
        amount: u32,
    },
    HpLost {
        target: TargetRef,
        amount: u32,
    },
    StatusApplied {
        target: TargetRef,
        status: StatusKind,
        amount: i32,
    },
    StatusTicked {
        target: TargetRef,
        status: StatusKind,
        remaining: u32,
    },
    StatusExpired {
        target: TargetRef,
        status: StatusKind,
    },
    EnemyMoved {
        index: usize,
        mv: EnemyMove,
    },
    IntentSet {
        index: usize,
        intent: IntentKind,
    },
    EnemyDied {
        index: usize,
    },
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
                let bite_damage = match sp {
                    Species::BarrowRat | Species::FenRat => rng.range(5, 7),
                    Species::MireCrawler => 6,
                    _ => 0,
                };
                let curl_up = is_rat.then(|| rng.range(3, 7));
                Enemy {
                    species: sp,
                    hp,
                    max_hp: hp,
                    block: 0,
                    statuses: Statuses {
                        curl_up,
                        ..Default::default()
                    },
                    bite_damage,
                    next_move: EnemyMove::Chant, // placeholder, rolled below
                    history: Vec::new(),
                }
            })
            .collect();

        let mut state = CombatState {
            player: Player {
                hp,
                max_hp,
                block: 0,
                energy: ENERGY_PER_TURN,
                statuses: Statuses::default(),
            },
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
            events.push(CombatEvent::IntentSet {
                index: i,
                intent: state.intent_of(i),
            });
        }

        events.push(CombatEvent::TurnStarted { turn: 1 });
        events.push(CombatEvent::EnergySet {
            energy: ENERGY_PER_TURN,
        });
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
            attack_damage(
                base,
                e.statuses.strength,
                e.statuses.weak > 0,
                self.player.statuses.vulnerable > 0,
            )
        };
        match e.next_move {
            EnemyMove::DarkStrike => IntentKind::Attack {
                damage: atk(6),
                hits: 1,
            },
            EnemyMove::Chomp => IntentKind::Attack {
                damage: atk(11),
                hits: 1,
            },
            EnemyMove::Rush => IntentKind::Attack {
                damage: atk(14),
                hits: 1,
            },
            EnemyMove::SkullBash => IntentKind::Attack {
                damage: atk(6),
                hits: 1,
            },
            EnemyMove::Bite => IntentKind::Attack {
                damage: atk(e.bite_damage),
                hits: 1,
            },
            EnemyMove::Thrash => IntentKind::AttackDefend { damage: atk(7) },
            EnemyMove::Chant | EnemyMove::Bellow | EnemyMove::Grow | EnemyMove::TrollBellow => {
                IntentKind::Buff
            }
            EnemyMove::Spittle => IntentKind::Debuff,
            EnemyMove::Stab => IntentKind::Attack {
                damage: atk(12),
                hits: 1,
            },
            EnemyMove::Rend => IntentKind::Attack {
                damage: atk(8),
                hits: 1,
            },
            EnemyMove::Maul => IntentKind::Attack {
                damage: atk(18),
                hits: 1,
            },
            EnemyMove::Screech => IntentKind::Attack {
                damage: atk(5),
                hits: 1,
            },
            EnemyMove::CrushingBlow => IntentKind::Attack {
                damage: atk(22),
                hits: 1,
            },
            EnemyMove::Peck => IntentKind::Attack {
                damage: atk(4),
                hits: 2,
            },
            EnemyMove::Cleave => IntentKind::Attack {
                damage: atk(8),
                hits: 2,
            },
            EnemyMove::GraveCleave => IntentKind::Attack {
                damage: atk(6),
                hits: 3,
            },
            EnemyMove::Bulwark => IntentKind::AttackDefend { damage: atk(10) },
            // DreadRoar both buffs (Strength) and debuffs (player Vulnerable); shown
            // as Buff because the Strength gain is the primary, lasting threat.
            EnemyMove::Fester | EnemyMove::WarChant | EnemyMove::DreadRoar => IntentKind::Buff,
            EnemyMove::SoulDrain => IntentKind::Debuff,
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
        events.push(CombatEvent::EnergySet {
            energy: self.player.energy,
        });

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
        self.attack_enemy_str(i, base, self.player.statuses.strength, events);
    }

    /// Attack with an explicit Strength (Body Slam passes 0).
    fn attack_enemy_str(&mut self, i: usize, base: u32, strength: i32, events: &mut Vec<CombatEvent>) {
        let dmg = attack_damage(
            base,
            strength,
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
                events.push(CombatEvent::BlockGained {
                    target: TargetRef::Enemy(i),
                    amount: curl,
                });
                events.push(CombatEvent::StatusExpired {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::CurlUp,
                });
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
                events.push(CombatEvent::IntentSet {
                    index: i,
                    intent: self.intent_of(i),
                });
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
                events.push(CombatEvent::BlockGained {
                    target: TargetRef::Player,
                    amount: n,
                });
            }
            Effect::DamageAll(base) => {
                let targets: Vec<usize> = self.living().collect();
                for i in targets {
                    if self.over.is_some() {
                        break;
                    }
                    if self.enemies[i].alive() {
                        self.attack_enemy(i, base, events);
                    }
                }
            }
            Effect::ApplyVulnerable(n) => {
                if let Some(t) = target {
                    if self.enemies[t].alive() {
                        self.enemies[t].statuses.vulnerable += n;
                        events.push(CombatEvent::StatusApplied {
                            target: TargetRef::Enemy(t),
                            status: StatusKind::Vulnerable,
                            amount: n as i32,
                        });
                    }
                }
            }
            Effect::ApplyVulnerableAll(n) => {
                let targets: Vec<usize> = self.living().collect();
                for i in targets {
                    self.enemies[i].statuses.vulnerable += n;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Enemy(i),
                        status: StatusKind::Vulnerable,
                        amount: n as i32,
                    });
                }
            }
            Effect::Draw(n) => {
                for _ in 0..n {
                    self.draw_one(rng, events);
                }
            }
            Effect::GainStrength(n) => {
                self.player.statuses.strength += n;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Strength,
                    amount: n,
                });
            }
            Effect::GainTempStrength(n) => {
                self.player.statuses.strength += n;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Strength,
                    amount: n,
                });
                if n > 0 {
                    self.player.statuses.strength_down += n as u32;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Player,
                        status: StatusKind::StrengthDown,
                        amount: n,
                    });
                }
            }
            Effect::AddCopyToDiscard => {
                self.discard.push(card);
                events.push(CombatEvent::CardAddedToDiscard { card });
            }
            Effect::ApplyWeak(n) => {
                if let Some(t) = target {
                    if self.enemies[t].alive() {
                        self.enemies[t].statuses.weak += n;
                        events.push(CombatEvent::StatusApplied {
                            target: TargetRef::Enemy(t),
                            status: StatusKind::Weak,
                            amount: n as i32,
                        });
                    }
                }
            }
            Effect::ApplyWeakAll(n) => {
                let targets: Vec<usize> = self.living().collect();
                for i in targets {
                    self.enemies[i].statuses.weak += n;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Enemy(i),
                        status: StatusKind::Weak,
                        amount: n as i32,
                    });
                }
            }
            Effect::DamageEqualToBlock => {
                if let Some(t) = target {
                    if self.enemies[t].alive() {
                        let base = self.player.block;
                        self.attack_enemy_str(t, base, 0, events);
                    }
                }
            }
            Effect::Heal(n) => {
                let healed = n.min(self.player.max_hp - self.player.hp);
                self.player.hp += healed;
                events.push(CombatEvent::Healed { target: TargetRef::Player, amount: healed });
            }
            Effect::LoseHp(n) => {
                let lost = n.min(self.player.hp);
                self.player.hp -= lost;
                events.push(CombatEvent::HpLost { target: TargetRef::Player, amount: lost });
            }
            Effect::GainEnergy(n) => {
                self.player.energy += n;
                events.push(CombatEvent::EnergySet { energy: self.player.energy });
            }
            Effect::GainRitual(n) => {
                self.player.statuses.ritual += n;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Ritual,
                    amount: n as i32,
                });
            }
            Effect::GainMetallicize(n) => {
                self.player.statuses.metallicize += n;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Metallicize,
                    amount: n as i32,
                });
            }
        }
    }

    fn end_turn(&mut self, rng: &mut RunRng) -> Vec<CombatEvent> {
        let mut events = Vec::new();

        // 1. Discard the hand.
        self.discard.append(&mut self.hand);
        events.push(CombatEvent::HandDiscarded);

        // 2. Player end-of-turn statuses.
        let sd = self.player.statuses.strength_down;
        if sd > 0 {
            self.player.statuses.strength -= sd as i32;
            self.player.statuses.strength_down = 0;
            events.push(CombatEvent::StatusApplied {
                target: TargetRef::Player,
                status: StatusKind::Strength,
                amount: -(sd as i32),
            });
            events.push(CombatEvent::StatusExpired {
                target: TargetRef::Player,
                status: StatusKind::StrengthDown,
            });
        }

        // Metallicize: gain Block at end of the player's turn (protects vs the enemy turn).
        if self.player.statuses.metallicize > 0 {
            let n = self.player.statuses.metallicize;
            self.player.block += n;
            events.push(CombatEvent::BlockGained { target: TargetRef::Player, amount: n });
        }

        // 3. Player duration tick.
        Self::push_tick_events(
            TargetRef::Player,
            self.player.statuses.tick_durations(),
            &mut events,
        );

        // 4. Enemy turns, in spawn order.
        for i in 0..self.enemies.len() {
            if !self.enemies[i].alive() {
                continue;
            }
            self.enemies[i].block = 0;
            events.push(CombatEvent::BlockReset {
                target: TargetRef::Enemy(i),
            });

            let mv = self.enemies[i].next_move;
            events.push(CombatEvent::EnemyMoved { index: i, mv });
            self.enemy_move(i, mv, &mut events);
            if self.over.is_some() {
                return events; // player died mid-round
            }

            let e = &mut self.enemies[i];
            if e.statuses.ritual > 0 {
                if e.statuses.ritual_fresh {
                    e.statuses.ritual_fresh = false;
                } else {
                    let gain = e.statuses.ritual as i32;
                    e.statuses.strength += gain;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Enemy(i),
                        status: StatusKind::Strength,
                        amount: gain,
                    });
                }
            }
            let ticks = e.statuses.tick_durations();
            Self::push_tick_events(TargetRef::Enemy(i), ticks, &mut events);
            self.enemies[i].history.push(mv);
        }

        // 5. Roll next moves and show intents.
        for i in 0..self.enemies.len() {
            if !self.enemies[i].alive() {
                continue;
            }
            self.enemies[i].next_move =
                roll_move(self.enemies[i].species, &self.enemies[i].history, rng);
            events.push(CombatEvent::IntentSet {
                index: i,
                intent: self.intent_of(i),
            });
        }

        // 6. New player turn.
        self.turn += 1;
        self.player.block = 0;
        events.push(CombatEvent::BlockReset {
            target: TargetRef::Player,
        });
        self.player.energy = ENERGY_PER_TURN;
        events.push(CombatEvent::EnergySet {
            energy: ENERGY_PER_TURN,
        });
        events.push(CombatEvent::TurnStarted { turn: self.turn });
        if self.player.statuses.ritual > 0 {
            let gain = self.player.statuses.ritual as i32;
            self.player.statuses.strength += gain;
            events.push(CombatEvent::StatusApplied {
                target: TargetRef::Player,
                status: StatusKind::Strength,
                amount: gain,
            });
        }
        for _ in 0..DRAW_PER_TURN {
            self.draw_one(rng, &mut events);
        }
        events
    }

    fn push_tick_events(
        target: TargetRef,
        ticks: Vec<(StatusKind, u32)>,
        events: &mut Vec<CombatEvent>,
    ) {
        for (status, remaining) in ticks {
            events.push(if remaining == 0 {
                CombatEvent::StatusExpired { target, status }
            } else {
                CombatEvent::StatusTicked {
                    target,
                    status,
                    remaining,
                }
            });
        }
    }

    fn enemy_move(&mut self, i: usize, mv: EnemyMove, events: &mut Vec<CombatEvent>) {
        match mv {
            EnemyMove::Chant => {
                let e = &mut self.enemies[i];
                e.statuses.ritual += 3;
                e.statuses.ritual_fresh = true;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Ritual,
                    amount: 3,
                });
            }
            EnemyMove::DarkStrike => self.enemy_attack(i, 6, events),
            EnemyMove::Chomp => self.enemy_attack(i, 11, events),
            EnemyMove::Thrash => {
                self.enemy_attack(i, 7, events);
                if self.over.is_none() {
                    self.enemies[i].block += 5;
                    events.push(CombatEvent::BlockGained {
                        target: TargetRef::Enemy(i),
                        amount: 5,
                    });
                }
            }
            EnemyMove::Bellow => {
                let e = &mut self.enemies[i];
                e.statuses.strength += 3;
                e.block += 6;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: 3,
                });
                events.push(CombatEvent::BlockGained {
                    target: TargetRef::Enemy(i),
                    amount: 6,
                });
            }
            EnemyMove::Bite => {
                let base = self.enemies[i].bite_damage;
                self.enemy_attack(i, base, events);
            }
            EnemyMove::Grow => {
                self.enemies[i].statuses.strength += 3;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: 3,
                });
            }
            EnemyMove::Spittle => {
                self.player.statuses.weak += 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Weak,
                    amount: 2,
                });
            }
            EnemyMove::TrollBellow => {
                self.enemies[i].statuses.enrage += 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Enrage,
                    amount: 2,
                });
            }
            EnemyMove::Rush => self.enemy_attack(i, 14, events),
            EnemyMove::SkullBash => {
                self.enemy_attack(i, 6, events);
                if self.over.is_none() {
                    self.player.statuses.vulnerable += 2;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Player,
                        status: StatusKind::Vulnerable,
                        amount: 2,
                    });
                }
            }
            EnemyMove::Stab => self.enemy_attack(i, 12, events),
            EnemyMove::Maul => self.enemy_attack(i, 18, events),
            EnemyMove::CrushingBlow => self.enemy_attack(i, 22, events),
            EnemyMove::Peck => self.enemy_attack_multi(i, 4, 2, events),
            EnemyMove::Cleave => self.enemy_attack_multi(i, 8, 2, events),
            EnemyMove::GraveCleave => self.enemy_attack_multi(i, 6, 3, events),
            EnemyMove::Rend => {
                self.enemy_attack(i, 8, events);
                if self.over.is_none() {
                    self.player.statuses.weak += 1;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Player,
                        status: StatusKind::Weak,
                        amount: 1,
                    });
                }
            }
            EnemyMove::Screech => {
                self.enemy_attack(i, 5, events);
                if self.over.is_none() {
                    self.player.statuses.vulnerable += 1;
                    events.push(CombatEvent::StatusApplied {
                        target: TargetRef::Player,
                        status: StatusKind::Vulnerable,
                        amount: 1,
                    });
                }
            }
            EnemyMove::Fester => {
                self.enemies[i].statuses.strength += 4;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: 4,
                });
            }
            EnemyMove::SoulDrain => {
                self.player.statuses.strength -= 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Strength,
                    amount: -2,
                });
            }
            EnemyMove::WarChant => {
                let e = &mut self.enemies[i];
                e.statuses.ritual += 2;
                e.statuses.ritual_fresh = true;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Ritual,
                    amount: 2,
                });
            }
            EnemyMove::DreadRoar => {
                self.enemies[i].statuses.strength += 3;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Enemy(i),
                    status: StatusKind::Strength,
                    amount: 3,
                });
                self.player.statuses.vulnerable += 2;
                events.push(CombatEvent::StatusApplied {
                    target: TargetRef::Player,
                    status: StatusKind::Vulnerable,
                    amount: 2,
                });
            }
            EnemyMove::Bulwark => {
                // Attack first, then block — mirrors Thrash, so the post-attack
                // effect is correctly skipped if the hit ends the combat.
                self.enemy_attack(i, 10, events);
                if self.over.is_none() {
                    self.enemies[i].block += 18;
                    events.push(CombatEvent::BlockGained {
                        target: TargetRef::Enemy(i),
                        amount: 18,
                    });
                }
            }
        }
    }

    fn enemy_attack(&mut self, i: usize, base: u32, events: &mut Vec<CombatEvent>) {
        let e = &self.enemies[i];
        let dmg = attack_damage(
            base,
            e.statuses.strength,
            e.statuses.weak > 0,
            self.player.statuses.vulnerable > 0,
        );
        let out = soak(&mut self.player.block, &mut self.player.hp, dmg);
        events.push(CombatEvent::DamageDealt {
            target: TargetRef::Player,
            amount: dmg,
            blocked: out.blocked,
            hp_lost: out.hp_lost,
        });
        if self.player.hp == 0 {
            self.over = Some(Outcome::Defeat);
            events.push(CombatEvent::PlayerDied);
        }
    }

    fn enemy_attack_multi(
        &mut self,
        i: usize,
        base: u32,
        hits: u32,
        events: &mut Vec<CombatEvent>,
    ) {
        for _ in 0..hits {
            if self.over.is_some() {
                return;
            }
            self.enemy_attack(i, base, events);
        }
    }
}

/// StS damage pipeline, integer math (truncating division == floor here):
/// strength adds to base, Weak multiplies ×0.75, Vulnerable ×1.5.
pub fn attack_damage(
    base: u32,
    strength: i32,
    attacker_weak: bool,
    target_vulnerable: bool,
) -> u32 {
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
    use crate::cards::{starter_deck, CardId, Effect};
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
            player: Player {
                hp: 80,
                max_hp: 80,
                block: 0,
                energy: 3,
                statuses: Statuses::default(),
            },
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
        events
            .iter()
            .filter(|e| matches!(e, CombatEvent::CardDrawn { .. }))
            .count()
    }

    #[test]
    fn new_combat_opens_with_5_cards_3_energy_and_intents() {
        let mut rng = RunRng::new(11);
        let (c, events) =
            CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        assert_eq!(c.hand.len(), 5);
        assert_eq!(c.draw.len(), 5);
        assert_eq!(c.player.energy, 3);
        assert_eq!(c.turn, 1);
        assert!(c.over.is_none());
        assert_eq!(count_drawn(&events), 5);
        assert!(events
            .iter()
            .any(|e| matches!(e, CombatEvent::TurnStarted { turn: 1 })));
        assert!(events
            .iter()
            .any(|e| matches!(e, CombatEvent::IntentSet { index: 0, .. })));
        // Wolf always opens with Chomp: base 11.
        assert_eq!(c.enemies[0].next_move, EnemyMove::Chomp);
        assert!(matches!(
            c.intent_of(0),
            IntentKind::Attack {
                damage: 11,
                hits: 1
            }
        ));
    }

    #[test]
    fn new_combat_rolls_hp_in_species_range() {
        let mut rng = RunRng::new(5);
        let (c, _) = CombatState::new(
            &mut rng,
            &starter_deck(),
            80,
            80,
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
        assert!(matches!(
            c.intent_of(0),
            IntentKind::Attack { damage: 6, hits: 1 }
        ));
        c.enemies[0].statuses.strength = 3;
        assert!(matches!(
            c.intent_of(0),
            IntentKind::Attack { damage: 9, hits: 1 }
        ));
        c.player.statuses.vulnerable = 1;
        assert!(matches!(
            c.intent_of(0),
            IntentKind::Attack {
                damage: 13,
                hits: 1
            }
        )); // floor(9*1.5)
    }

    fn play(
        c: &mut CombatState,
        i: usize,
        t: Option<usize>,
    ) -> Result<Vec<CombatEvent>, IllegalAction> {
        let mut rng = RunRng::new(0);
        c.apply(
            &mut rng,
            Action::PlayCard {
                hand_index: i,
                target: t,
            },
        )
    }

    #[test]
    fn hew_deals_6_costs_1_and_goes_to_discard() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::Hew]);
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 44);
        assert_eq!(c.player.energy, 2);
        assert!(c.hand.is_empty());
        assert_eq!(c.discard, vec![CardId::Hew]);
        assert!(events.contains(&CombatEvent::CardPlayed {
            card: CardId::Hew,
            hand_index: 0
        }));
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0),
            amount: 6,
            blocked: 0,
            hp_lost: 6
        }));
        assert!(events.contains(&CombatEvent::EnergySet { energy: 2 }));
    }

    #[test]
    fn raise_shield_gains_5_block() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50)],
            vec![CardId::RaiseShield],
        );
        let events = play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.block, 5);
        assert!(events.contains(&CombatEvent::BlockGained {
            target: TargetRef::Player,
            amount: 5
        }));
    }

    #[test]
    fn attack_consumes_enemy_block_first() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![CardId::Hew]);
        c.enemies[0].block = 4;
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 48);
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0),
            amount: 6,
            blocked: 4,
            hp_lost: 2
        }));
    }

    #[test]
    fn haft_strike_draws_a_card() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50)],
            vec![CardId::HaftStrike],
        );
        c.draw = vec![CardId::Hew];
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.hand, vec![CardId::Hew]);
        assert!(events.contains(&CombatEvent::CardDrawn { card: CardId::Hew }));
    }

    #[test]
    fn drawing_from_empty_pile_reshuffles_discard() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50)],
            vec![CardId::Unbowed],
        );
        c.discard = vec![CardId::Hew, CardId::RaiseShield];
        let events = play(&mut c, 0, None).unwrap();
        assert!(events.contains(&CombatEvent::DeckShuffled));
        assert_eq!(c.hand.len(), 1);
        assert_eq!(c.draw.len(), 1); // 2 reshuffled, 1 drawn
                                     // Unbowed itself goes to discard AFTER resolving, so it wasn't shuffled in.
        assert_eq!(c.discard, vec![CardId::Unbowed]);
    }

    #[test]
    fn draws_beyond_hand_limit_are_forfeited() {
        // A full hand at draw time is unreachable through Phase 1's card pool
        // (playing always frees a slot first), so this is the one place we
        // unit-test the private helper directly — same file, so tests can.
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)], vec![CardId::Hew; 10]);
        c.draw = vec![CardId::Hew];
        let mut rng = RunRng::new(0);
        let mut evs = Vec::new();
        c.draw_one(&mut rng, &mut evs);
        assert!(evs.is_empty(), "draw at hand limit is forfeited silently");
        assert_eq!(c.hand.len(), 10);
        assert_eq!(c.draw, vec![CardId::Hew], "the card stays on the draw pile");
    }

    #[test]
    fn whirling_axe_hits_all_living_enemies_only() {
        let mut c = combat_vs(
            vec![
                enemy(Species::BarrowRat, 12),
                enemy(Species::FenRat, 12),
                enemy(Species::GraveWolf, 40),
            ],
            vec![CardId::WhirlingAxe],
        );
        c.enemies[1].hp = 0;
        let events = play(&mut c, 0, None).unwrap();
        let hits = events
            .iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 2);
        assert_eq!(c.enemies[0].hp, 4); // the fixture has no Curl Up
        assert_eq!(c.enemies[2].hp, 32);
    }

    #[test]
    fn twin_axes_hits_twice_and_second_hit_fizzles_on_kill() {
        // Two enemies so the first kill doesn't end combat.
        let mut c = combat_vs(
            vec![
                enemy(Species::DraugrChanter, 4),
                enemy(Species::GraveWolf, 40),
            ],
            vec![CardId::TwinAxes],
        );
        let events = play(&mut c, 0, Some(0)).unwrap();
        let hits: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .collect();
        assert_eq!(hits.len(), 1, "second hit fizzles on a corpse");
        assert!(events.contains(&CombatEvent::EnemyDied { index: 0 }));
        assert!(c.over.is_none());
    }

    #[test]
    fn twin_axes_double_hits_a_survivor() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)], vec![CardId::TwinAxes]);
        let events = play(&mut c, 0, Some(0)).unwrap();
        let hits = events
            .iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 2);
        assert_eq!(c.enemies[0].hp, 30);
    }

    #[test]
    fn curl_up_triggers_once_between_hits() {
        let mut c = combat_vs(vec![enemy(Species::BarrowRat, 20)], vec![CardId::TwinAxes]);
        c.enemies[0].statuses.curl_up = Some(4);
        let events = play(&mut c, 0, Some(0)).unwrap();
        // Hit 1: 5 damage to HP (20→15), curl up grants 4 block.
        // Hit 2: 5 damage, 4 blocked, 1 to HP (15→14).
        assert_eq!(c.enemies[0].hp, 14);
        assert_eq!(c.enemies[0].block, 0);
        assert_eq!(c.enemies[0].statuses.curl_up, None);
        assert!(events.contains(&CombatEvent::BlockGained {
            target: TargetRef::Enemy(0),
            amount: 4
        }));
        assert!(events.contains(&CombatEvent::StatusExpired {
            target: TargetRef::Enemy(0),
            status: StatusKind::CurlUp
        }));
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Enemy(0),
            amount: 5,
            blocked: 4,
            hp_lost: 1
        }));
    }

    #[test]
    fn skull_splitter_applies_vulnerable_and_amplifies_followups() {
        let mut c = combat_vs(
            vec![enemy(Species::GraveWolf, 40)],
            vec![CardId::SkullSplitter, CardId::Hew],
        );
        play(&mut c, 0, Some(0)).unwrap(); // 8 damage + Vulnerable 2
        assert_eq!(c.enemies[0].hp, 32);
        assert_eq!(c.enemies[0].statuses.vulnerable, 2);
        play(&mut c, 0, Some(0)).unwrap(); // Hew: floor(6*1.5) = 9
        assert_eq!(c.enemies[0].hp, 23);
    }

    #[test]
    fn thors_wrath_damages_and_debuffs_all() {
        let mut c = combat_vs(
            vec![enemy(Species::BarrowRat, 20), enemy(Species::FenRat, 20)],
            vec![CardId::ThorsWrath],
        );
        play(&mut c, 0, None).unwrap();
        for e in &c.enemies {
            assert_eq!(e.statuses.vulnerable, 1);
        }
        assert_eq!(c.enemies[0].hp, 16);
        assert_eq!(c.enemies[1].hp, 16);
    }

    #[test]
    fn surge_of_rage_gives_temporary_strength() {
        let mut c = combat_vs(
            vec![enemy(Species::GraveWolf, 40)],
            vec![CardId::SurgeOfRage, CardId::Hew],
        );
        let events = play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.statuses.strength, 2);
        assert_eq!(c.player.statuses.strength_down, 2);
        assert!(events.contains(&CombatEvent::StatusApplied {
            target: TargetRef::Player,
            status: StatusKind::Strength,
            amount: 2
        }));
        play(&mut c, 0, Some(0)).unwrap(); // Hew: 6+2 = 8
        assert_eq!(c.enemies[0].hp, 32);
    }

    #[test]
    fn berserkergang_is_consumed_and_strength_persists() {
        let mut c = combat_vs(
            vec![enemy(Species::GraveWolf, 40)],
            vec![CardId::Berserkergang, CardId::Hew],
        );
        play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.statuses.strength, 2);
        assert_eq!(c.player.statuses.strength_down, 0);
        assert!(c.discard.is_empty(), "powers are consumed, not discarded");
        play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 32);
    }

    #[test]
    fn rising_fury_adds_a_copy_to_discard() {
        let mut c = combat_vs(
            vec![enemy(Species::GraveWolf, 40)],
            vec![CardId::RisingFury],
        );
        let energy_before = c.player.energy;
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.player.energy, energy_before, "Rising Fury costs 0");
        assert_eq!(c.enemies[0].hp, 34);
        // Copy added during resolution + the played card itself.
        assert_eq!(c.discard, vec![CardId::RisingFury, CardId::RisingFury]);
        assert!(events.contains(&CombatEvent::CardAddedToDiscard {
            card: CardId::RisingFury
        }));
    }

    #[test]
    fn enrage_triggers_on_skills_and_updates_intent() {
        let mut c = combat_vs(
            vec![enemy(Species::ForestTroll, 84)],
            vec![CardId::RaiseShield, CardId::Hew],
        );
        c.enemies[0].statuses.enrage = 2;
        c.enemies[0].next_move = EnemyMove::Rush;
        let events = play(&mut c, 0, None).unwrap(); // skill
        assert_eq!(c.enemies[0].statuses.strength, 2);
        assert!(events.contains(&CombatEvent::StatusApplied {
            target: TargetRef::Enemy(0),
            status: StatusKind::Strength,
            amount: 2
        }));
        assert!(events.contains(&CombatEvent::IntentSet {
            index: 0,
            intent: IntentKind::Attack {
                damage: 16,
                hits: 1
            }, // 14 + 2
        }));
        let events = play(&mut c, 0, Some(0)).unwrap(); // attack: no enrage
        assert_eq!(c.enemies[0].statuses.strength, 2);
        assert!(!events.iter().any(|e| matches!(
            e,
            CombatEvent::StatusApplied {
                status: StatusKind::Strength,
                ..
            }
        )));
    }

    #[test]
    fn killing_the_last_enemy_wins_and_locks_the_combat() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 5)],
            vec![CardId::Hew, CardId::Hew],
        );
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.over, Some(Outcome::Victory));
        assert!(events.contains(&CombatEvent::EnemyDied { index: 0 }));
        assert!(events.contains(&CombatEvent::Victory));
        assert_eq!(play(&mut c, 0, Some(0)), Err(IllegalAction::CombatOver));
        let mut rng = RunRng::new(0);
        assert_eq!(
            c.apply(&mut rng, Action::EndTurn),
            Err(IllegalAction::CombatOver)
        );
    }

    #[test]
    fn skull_splitter_kill_skips_the_vulnerable_application() {
        // Victory stops remaining effects: the kill comes first in the list.
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 8)],
            vec![CardId::SkullSplitter],
        );
        let events = play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.over, Some(Outcome::Victory));
        assert!(!events.iter().any(|e| matches!(
            e,
            CombatEvent::StatusApplied {
                status: StatusKind::Vulnerable,
                ..
            }
        )));
    }

    #[test]
    fn thors_wrath_kill_skips_the_aoe_vulnerable() {
        // DamageAll kills the last enemy: Victory stops ApplyVulnerableAll.
        let mut c = combat_vs(vec![enemy(Species::BarrowRat, 3)], vec![CardId::ThorsWrath]);
        let events = play(&mut c, 0, None).unwrap();
        assert_eq!(c.over, Some(Outcome::Victory));
        assert!(!events.iter().any(|e| matches!(
            e,
            CombatEvent::StatusApplied {
                status: StatusKind::Vulnerable,
                ..
            }
        )));
    }

    #[test]
    fn not_enough_energy_is_rejected() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50)],
            vec![CardId::SkullSplitter],
        );
        c.player.energy = 1; // Skull-Splitter costs 2
        assert_eq!(
            play(&mut c, 0, Some(0)),
            Err(IllegalAction::NotEnoughEnergy)
        );
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

    fn end_turn(c: &mut CombatState, seed: u64) -> Vec<CombatEvent> {
        let mut rng = RunRng::new(seed);
        c.apply(&mut rng, Action::EndTurn).unwrap()
    }

    #[test]
    fn end_turn_discards_hand_and_refills_everything() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50)],
            vec![CardId::Hew, CardId::RaiseShield],
        );
        c.draw = starter_deck();
        c.player.energy = 0;
        c.player.block = 7;
        let events = end_turn(&mut c, 1);
        assert!(events.contains(&CombatEvent::HandDiscarded));
        assert_eq!(c.turn, 2);
        assert_eq!(c.player.energy, 3);
        assert_eq!(c.player.block, 0, "player block expires at turn start");
        assert_eq!(c.hand.len(), 5);
        assert!(c.discard.contains(&CardId::RaiseShield));
        assert!(events.contains(&CombatEvent::TurnStarted { turn: 2 }));
        assert!(events
            .iter()
            .any(|e| matches!(e, CombatEvent::IntentSet { .. })));
    }

    #[test]
    fn enemy_attacks_through_block() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        c.player.block = 4; // DarkStrike hits 6: 4 blocked, 2 HP lost
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 78);
        assert!(events.contains(&CombatEvent::DamageDealt {
            target: TargetRef::Player,
            amount: 6,
            blocked: 4,
            hp_lost: 2
        }));
        assert!(events.contains(&CombatEvent::EnemyMoved {
            index: 0,
            mv: EnemyMove::DarkStrike
        }));
    }

    #[test]
    fn ritual_skips_its_first_turn_then_scales_6_9_12() {
        // Fresh chanter exactly as a real fight starts.
        let mut c = combat_vs(
            vec![Enemy {
                history: vec![],
                next_move: EnemyMove::Chant,
                ..enemy(Species::DraugrChanter, 54)
            }],
            vec![],
        );
        let hp0 = c.player.hp;
        end_turn(&mut c, 1); // chant; ritual fresh: no strength yet
        assert_eq!(c.player.hp, hp0);
        assert_eq!(c.enemies[0].statuses.ritual, 3);
        assert_eq!(c.enemies[0].statuses.strength, 0);
        end_turn(&mut c, 2); // attacks 6; then end-of-its-turn: +3
        assert_eq!(c.player.hp, hp0 - 6);
        assert_eq!(c.enemies[0].statuses.strength, 3);
        end_turn(&mut c, 3); // attacks 9
        assert_eq!(c.player.hp, hp0 - 15);
        end_turn(&mut c, 4); // attacks 12
        assert_eq!(c.player.hp, hp0 - 27);
    }

    #[test]
    fn fen_rat_spittle_weakens_the_player() {
        let mut c = combat_vs(
            vec![Enemy {
                next_move: EnemyMove::Spittle,
                ..enemy(Species::FenRat, 14)
            }],
            vec![CardId::Hew],
        );
        end_turn(&mut c, 1);
        assert_eq!(c.player.statuses.weak, 2);
        // Weak player: Hew deals floor(6*0.75) = 4.
        play(&mut c, 0, Some(0)).unwrap();
        assert_eq!(c.enemies[0].hp, 10);
    }

    #[test]
    fn player_durations_tick_at_end_of_player_turn() {
        let mut c = combat_vs(vec![enemy(Species::DraugrChanter, 50)], vec![]);
        c.player.statuses.vulnerable = 2;
        // End turn: tick 2→1 (before the enemy acts), enemy hits 6*1.5 = 9.
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 71);
        assert!(events.contains(&CombatEvent::StatusTicked {
            target: TargetRef::Player,
            status: StatusKind::Vulnerable,
            remaining: 1
        }));
        // Next end turn: tick 1→0, enemy hits plain 6.
        let events = end_turn(&mut c, 2);
        assert_eq!(c.player.hp, 65);
        assert!(events.contains(&CombatEvent::StatusExpired {
            target: TargetRef::Player,
            status: StatusKind::Vulnerable
        }));
    }

    #[test]
    fn enemy_durations_tick_at_end_of_its_own_turn() {
        let mut c = combat_vs(vec![enemy(Species::GraveWolf, 40)], vec![]);
        c.enemies[0].statuses.vulnerable = 2;
        c.enemies[0].next_move = EnemyMove::Bellow;
        let events = end_turn(&mut c, 1);
        assert_eq!(c.enemies[0].statuses.vulnerable, 1);
        assert!(events.contains(&CombatEvent::StatusTicked {
            target: TargetRef::Enemy(0),
            status: StatusKind::Vulnerable,
            remaining: 1
        }));
    }

    #[test]
    fn strength_down_fires_at_end_of_player_turn() {
        let mut c = combat_vs(
            vec![enemy(Species::DraugrChanter, 50)],
            vec![CardId::SurgeOfRage],
        );
        play(&mut c, 0, None).unwrap();
        assert_eq!(c.player.statuses.strength, 2);
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.statuses.strength, 0);
        assert_eq!(c.player.statuses.strength_down, 0);
        assert!(events.contains(&CombatEvent::StatusApplied {
            target: TargetRef::Player,
            status: StatusKind::Strength,
            amount: -2
        }));
        assert!(events.contains(&CombatEvent::StatusExpired {
            target: TargetRef::Player,
            status: StatusKind::StrengthDown
        }));
    }

    #[test]
    fn thrash_attacks_and_blocks_and_bellow_buffs() {
        let mut c = combat_vs(
            vec![Enemy {
                next_move: EnemyMove::Thrash,
                ..enemy(Species::GraveWolf, 40)
            }],
            vec![],
        );
        end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 73); // 7 damage
                                     // Wolf's block was gained AFTER its block reset, so it shows 5 now.
        assert_eq!(c.enemies[0].block, 5);
        // Next turn the wolf's own block resets first.
        c.enemies[0].next_move = EnemyMove::Bellow;
        end_turn(&mut c, 2);
        assert_eq!(c.enemies[0].block, 6, "old 5 reset, Bellow grants 6");
        assert_eq!(c.enemies[0].statuses.strength, 3);
    }

    #[test]
    fn troll_bellow_applies_enrage() {
        let mut c = combat_vs(
            vec![Enemy {
                history: vec![],
                next_move: EnemyMove::TrollBellow,
                ..enemy(Species::ForestTroll, 84)
            }],
            vec![],
        );
        end_turn(&mut c, 1);
        assert_eq!(c.enemies[0].statuses.enrage, 2);
    }

    #[test]
    fn player_death_stops_the_round_immediately() {
        let mut c = combat_vs(
            vec![
                enemy(Species::DraugrChanter, 50),
                enemy(Species::DraugrChanter, 50),
            ],
            vec![],
        );
        c.player.hp = 3; // first DarkStrike (6) kills
        let events = end_turn(&mut c, 1);
        assert_eq!(c.over, Some(Outcome::Defeat));
        assert!(events.contains(&CombatEvent::PlayerDied));
        let hits = events
            .iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 1, "second enemy never acts");
        assert!(!events
            .iter()
            .any(|e| matches!(e, CombatEvent::TurnStarted { turn: 2 })));
    }

    #[test]
    fn dead_enemies_are_skipped() {
        let mut c = combat_vs(
            vec![
                enemy(Species::DraugrChanter, 50),
                enemy(Species::DraugrChanter, 50),
            ],
            vec![],
        );
        c.enemies[0].hp = 0;
        let events = end_turn(&mut c, 1);
        let hits = events
            .iter()
            .filter(|e| matches!(e, CombatEvent::DamageDealt { .. }))
            .count();
        assert_eq!(hits, 1);
        assert!(!events
            .iter()
            .any(|e| matches!(e, CombatEvent::IntentSet { index: 0, .. })));
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

    #[test]
    fn warlord_cleave_hits_twice() {
        let mut e = enemy(Species::DraugrWarlord, 90);
        e.next_move = EnemyMove::Cleave;
        e.history = vec![EnemyMove::WarChant];
        let mut c = combat_vs(vec![e], vec![]);
        c.player.hp = 80;
        end_turn(&mut c, 1);
        // Cleave = 8 x2 = 16 to an unblocked player.
        assert_eq!(c.player.hp, 64);
    }

    #[test]
    fn soul_drain_saps_player_strength() {
        let mut e = enemy(Species::BarrowWight, 88);
        e.next_move = EnemyMove::SoulDrain;
        let mut c = combat_vs(vec![e], vec![]);
        end_turn(&mut c, 1);
        assert_eq!(c.player.statuses.strength, -2);
    }

    #[test]
    fn jarl_dread_roar_buffs_and_debuffs() {
        let mut e = enemy(Species::MoundJarl, 150);
        e.next_move = EnemyMove::DreadRoar;
        e.history = vec![];
        let mut c = combat_vs(vec![e], vec![]);
        end_turn(&mut c, 1);
        assert_eq!(c.enemies[0].statuses.strength, 3);
        assert_eq!(c.player.statuses.vulnerable, 2);
    }

    #[test]
    fn multi_hit_stops_if_player_dies_midway() {
        let mut e = enemy(Species::MoundJarl, 150);
        e.next_move = EnemyMove::GraveCleave; // 6 x3
        let mut c = combat_vs(vec![e], vec![]);
        c.player.hp = 5; // first 6-hit kills
        let events = end_turn(&mut c, 1);
        assert_eq!(c.player.hp, 0);
        assert_eq!(c.over, Some(Outcome::Defeat));
        // Exactly one damage event reached the player (no hits after death).
        let dmg = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    CombatEvent::DamageDealt {
                        target: TargetRef::Player,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(dmg, 1);
    }

    #[test]
    fn multi_hit_intent_reports_hits() {
        let mut e = enemy(Species::DraugrWarlord, 90);
        e.next_move = EnemyMove::Cleave;
        let c = combat_vs(vec![e], vec![]);
        assert!(matches!(
            c.intent_of(0),
            IntentKind::Attack { damage: 8, hits: 2 }
        ));
    }

    #[test]
    fn apply_weak_sets_enemy_weak() {
        let mut rng = RunRng::new(0);
        let (mut c, _) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        c.run_effect(&mut RunRng::new(0), Effect::ApplyWeak(2), CardId::Hew, Some(0), &mut vec![]);
        assert_eq!(c.enemies[0].statuses.weak, 2);
    }

    #[test]
    fn damage_equal_to_block_uses_block_not_strength() {
        let mut rng = RunRng::new(0);
        let (mut c, _) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        c.player.block = 7;
        c.player.statuses.strength = 3; // must NOT add
        let before = c.enemies[0].hp;
        c.run_effect(&mut RunRng::new(0), Effect::DamageEqualToBlock, CardId::Hew, Some(0), &mut vec![]);
        assert_eq!(before - c.enemies[0].hp, 7, "deals Block (7), Strength ignored");
    }

    #[test]
    fn heal_caps_and_losehp_ignores_block() {
        let mut rng = RunRng::new(0);
        let (mut c, _) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        c.player.hp = c.player.max_hp - 2;
        c.run_effect(&mut RunRng::new(0), Effect::Heal(10), CardId::Hew, None, &mut vec![]);
        assert_eq!(c.player.hp, c.player.max_hp);
        c.player.block = 50;
        c.run_effect(&mut RunRng::new(0), Effect::LoseHp(5), CardId::Hew, None, &mut vec![]);
        assert_eq!(c.player.hp, c.player.max_hp - 5, "LoseHp bypasses Block");
    }

    #[test]
    fn gain_energy_adds_energy() {
        let mut rng = RunRng::new(0);
        let (mut c, _) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        c.player.energy = 1;
        c.run_effect(&mut RunRng::new(0), Effect::GainEnergy(2), CardId::Hew, None, &mut vec![]);
        assert_eq!(c.player.energy, 3);
    }

    #[test]
    fn player_ritual_grants_strength_at_start_of_next_turn() {
        let mut rng = RunRng::new(0);
        let (mut c, _) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        c.run_effect(&mut RunRng::new(0), Effect::GainRitual(2), CardId::Hew, None, &mut vec![]);
        let before = c.player.statuses.strength;
        let _ = c.end_turn(&mut RunRng::new(0)); // advances to the next player turn
        assert_eq!(c.player.statuses.strength, before + 2, "gains Strength at the new turn");
    }

    #[test]
    fn metallicize_grants_block_at_end_of_player_turn() {
        let mut rng = RunRng::new(0);
        let (mut c, _) = CombatState::new(&mut rng, &starter_deck(), 80, 80, &[Species::GraveWolf]);
        c.run_effect(&mut RunRng::new(0), Effect::GainMetallicize(4), CardId::Hew, None, &mut vec![]);
        let evs = c.end_turn(&mut RunRng::new(0));
        assert!(evs.iter().any(|e| matches!(e,
            CombatEvent::BlockGained { target: TargetRef::Player, amount: 4 })));
    }
}
