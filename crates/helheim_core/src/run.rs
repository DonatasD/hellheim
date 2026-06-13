use crate::cards::{starter_deck, CardId, REWARD_POOL};
use crate::combat::{Action, CombatEvent, CombatState, IllegalAction, Outcome, TargetRef};
use crate::enemies::Species;
use crate::rng::RunRng;

pub const STARTING_HP: u32 = 80;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Fight(u8),
    Reward { after_fight: u8, offer: [CardId; 3] },
    Victory,
    Defeat,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct RunStats {
    pub turns: u32,
    pub damage_dealt: u64,
    pub damage_taken: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunError { NotInFight, NotInReward, BadIndex }

/// One whole gauntlet run: master deck, carried HP, stage progression, and
/// the single RNG stream that makes the run reproducible from its seed.
pub struct RunState {
    pub seed: u64,
    rng: RunRng,
    pub master_deck: Vec<CardId>,
    pub hp: u32,
    pub max_hp: u32,
    pub stage: Stage,
    pub combat: Option<CombatState>,
    pub stats: RunStats,
}

impl RunState {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rng: RunRng::new(seed),
            master_deck: starter_deck(),
            hp: STARTING_HP,
            max_hp: STARTING_HP,
            stage: Stage::Fight(1),
            combat: None,
            stats: RunStats::default(),
        }
    }

    fn encounter(&mut self, fight: u8) -> Vec<Species> {
        match fight {
            1 => vec![self.rng.pick(&[Species::DraugrChanter, Species::GraveWolf])],
            2 => vec![Species::BarrowRat, Species::FenRat],
            _ => vec![Species::ForestTroll],
        }
    }

    pub fn begin_fight(&mut self) -> Result<Vec<CombatEvent>, RunError> {
        let Stage::Fight(n) = self.stage else {
            return Err(RunError::NotInFight);
        };
        if self.combat.is_some() {
            return Err(RunError::NotInFight);
        }
        let species = self.encounter(n);
        let (combat, events) =
            CombatState::new(&mut self.rng, &self.master_deck, self.hp, self.max_hp, &species);
        self.combat = Some(combat);
        self.track(&events);
        Ok(events)
    }

    pub fn apply(&mut self, action: Action) -> Result<Vec<CombatEvent>, IllegalAction> {
        let combat = self.combat.as_mut().ok_or(IllegalAction::CombatOver)?;
        let events = combat.apply(&mut self.rng, action)?;
        self.track(&events);

        match self.combat.as_ref().and_then(|c| c.over) {
            Some(Outcome::Victory) => {
                let combat = self.combat.take().expect("combat exists");
                self.hp = combat.player.hp;
                let Stage::Fight(n) = self.stage else { unreachable!("victory outside a fight") };
                if n >= 3 {
                    self.stage = Stage::Victory;
                } else {
                    let offer = self.roll_offer();
                    self.stage = Stage::Reward { after_fight: n, offer };
                }
            }
            Some(Outcome::Defeat) => {
                self.combat = None;
                self.hp = 0;
                self.stage = Stage::Defeat;
            }
            None => {}
        }
        Ok(events)
    }

    pub fn choose_reward(&mut self, pick: Option<usize>) -> Result<(), RunError> {
        let Stage::Reward { after_fight, offer } = self.stage else {
            return Err(RunError::NotInReward);
        };
        if let Some(i) = pick {
            let card = *offer.get(i).ok_or(RunError::BadIndex)?;
            self.master_deck.push(card);
        }
        self.stage = Stage::Fight(after_fight + 1);
        Ok(())
    }

    fn roll_offer(&mut self) -> [CardId; 3] {
        let mut pool = REWARD_POOL.to_vec();
        self.rng.shuffle(&mut pool);
        [pool[0], pool[1], pool[2]]
    }

    fn track(&mut self, events: &[CombatEvent]) {
        for ev in events {
            match ev {
                CombatEvent::TurnStarted { .. } => self.stats.turns += 1,
                CombatEvent::DamageDealt { target, amount, .. } => match target {
                    TargetRef::Player => self.stats.damage_taken += u64::from(*amount),
                    TargetRef::Enemy(_) => self.stats.damage_dealt += u64::from(*amount),
                },
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::REWARD_POOL;
    use crate::combat::Action;
    use crate::enemies::Species;

    /// Reach into the public combat state and rig every enemy to 1 HP, then
    /// bot through: play the first affordable attack at the first living
    /// enemy, else end turn. Wins fast and exercises the real API.
    fn win_current_fight(run: &mut RunState) {
        for e in &mut run.combat.as_mut().unwrap().enemies {
            e.hp = 1;
        }
        for _ in 0..200 {
            if run.combat.is_none() {
                return; // stage advanced
            }
            let action = {
                let c = run.combat.as_ref().unwrap();
                let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
                c.hand
                    .iter()
                    .enumerate()
                    .find_map(|(i, card)| {
                        let spec = card.spec();
                        if spec.cost > c.player.energy {
                            return None;
                        }
                        match spec.targeting {
                            crate::cards::Targeting::SingleEnemy => target.map(|t| {
                                Action::PlayCard { hand_index: i, target: Some(t) }
                            }),
                            _ => Some(Action::PlayCard { hand_index: i, target: None }),
                        }
                    })
                    .unwrap_or(Action::EndTurn)
            };
            run.apply(action).unwrap();
        }
        panic!("rigged fight did not end in 200 actions");
    }

    #[test]
    fn new_run_starts_at_fight_1_with_starter_deck() {
        let run = RunState::new(7);
        assert_eq!(run.stage, Stage::Fight(1));
        assert_eq!(run.master_deck.len(), 10);
        assert_eq!(run.hp, 80);
        assert!(run.combat.is_none());
    }

    #[test]
    fn fight_1_is_chanter_or_wolf_fight_2_rats_fight_3_troll() {
        let mut run = RunState::new(7);
        run.begin_fight().unwrap();
        {
            let c = run.combat.as_ref().unwrap();
            assert_eq!(c.enemies.len(), 1);
            assert!(matches!(c.enemies[0].species,
                Species::DraugrChanter | Species::GraveWolf));
        }
        win_current_fight(&mut run);
        run.choose_reward(None).unwrap();
        run.begin_fight().unwrap();
        {
            let c = run.combat.as_ref().unwrap();
            let species: Vec<Species> = c.enemies.iter().map(|e| e.species).collect();
            assert_eq!(species, vec![Species::BarrowRat, Species::FenRat]);
        }
        win_current_fight(&mut run);
        run.choose_reward(None).unwrap();
        run.begin_fight().unwrap();
        assert_eq!(run.combat.as_ref().unwrap().enemies[0].species, Species::ForestTroll);
    }

    #[test]
    fn winning_a_fight_offers_3_distinct_pool_cards() {
        let mut run = RunState::new(11);
        run.begin_fight().unwrap();
        win_current_fight(&mut run);
        let Stage::Reward { after_fight, offer } = run.stage else {
            panic!("expected reward stage, got {:?}", run.stage)
        };
        assert_eq!(after_fight, 1);
        assert!(offer.iter().all(|c| REWARD_POOL.contains(c)));
        assert_ne!(offer[0], offer[1]);
        assert_ne!(offer[1], offer[2]);
        assert_ne!(offer[0], offer[2]);
    }

    #[test]
    fn choosing_a_reward_grows_the_master_deck_skipping_does_not() {
        let mut run = RunState::new(11);
        run.begin_fight().unwrap();
        win_current_fight(&mut run);
        let Stage::Reward { offer, .. } = run.stage else { panic!() };
        run.choose_reward(Some(1)).unwrap();
        assert_eq!(run.master_deck.len(), 11);
        assert_eq!(*run.master_deck.last().unwrap(), offer[1]);
        assert_eq!(run.stage, Stage::Fight(2));

        let mut run2 = RunState::new(11);
        run2.begin_fight().unwrap();
        win_current_fight(&mut run2);
        run2.choose_reward(None).unwrap();
        assert_eq!(run2.master_deck.len(), 10);
        assert_eq!(run2.stage, Stage::Fight(2));
    }

    #[test]
    fn reward_errors_in_wrong_stage_or_bad_index() {
        let mut run = RunState::new(3);
        assert_eq!(run.choose_reward(Some(0)), Err(RunError::NotInReward));
        run.begin_fight().unwrap();
        win_current_fight(&mut run);
        assert_eq!(run.choose_reward(Some(9)), Err(RunError::BadIndex));
    }

    #[test]
    fn hp_carries_between_fights() {
        let mut run = RunState::new(5);
        run.begin_fight().unwrap();
        run.combat.as_mut().unwrap().player.hp = 55; // pretend we took 25
        win_current_fight(&mut run);
        assert_eq!(run.hp, 55);
        run.choose_reward(None).unwrap();
        run.begin_fight().unwrap();
        assert_eq!(run.combat.as_ref().unwrap().player.hp, 55);
    }

    #[test]
    fn beating_fight_3_wins_the_run() {
        let mut run = RunState::new(13);
        for fight in 1..=3u8 {
            assert_eq!(run.stage, Stage::Fight(fight));
            run.begin_fight().unwrap();
            win_current_fight(&mut run);
            if fight < 3 {
                run.choose_reward(Some(0)).unwrap();
            }
        }
        assert_eq!(run.stage, Stage::Victory);
        assert_eq!(run.master_deck.len(), 12);
        assert!(run.stats.damage_dealt > 0);
        assert!(run.stats.turns > 0);
    }

    #[test]
    fn dying_sets_defeat_and_zero_hp() {
        let mut run = RunState::new(17);
        run.begin_fight().unwrap();
        run.combat.as_mut().unwrap().player.hp = 1;
        for _ in 0..20 {
            if run.combat.is_none() {
                break;
            }
            run.apply(Action::EndTurn).unwrap();
        }
        assert_eq!(run.stage, Stage::Defeat);
        assert_eq!(run.hp, 0);
    }

    #[test]
    fn begin_fight_guards_stage_and_double_start() {
        let mut run = RunState::new(1);
        run.begin_fight().unwrap();
        assert_eq!(run.begin_fight(), Err(RunError::NotInFight));
    }
}
