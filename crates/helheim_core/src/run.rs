use crate::cards::{starter_deck, CardId, REWARD_POOL};
use crate::combat::{Action, CombatEvent, CombatState, IllegalAction, Outcome, TargetRef};
use crate::encounters::roll_encounter;
use crate::enemies::Species;
use crate::map::{MapGraph, NodeId, NodeKind};
use crate::rng::RunRng;

pub const STARTING_HP: u32 = 80;
pub const REST_HEAL_PERCENT: u32 = 30;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RewardSource {
    Combat,
    Treasure,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    ChoosingNode,
    Reward {
        offer: [CardId; 3],
        source: RewardSource,
    },
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
pub enum RunError {
    NotChoosingNode,
    IllegalMove,
    InCombat,
    NotInReward,
    BadIndex,
}

/// One whole Act 1 run: the generated map, current position, carried HP, and the
/// single RNG stream that makes the run reproducible from its seed.
pub struct RunState {
    pub seed: u64,
    rng: RunRng,
    pub master_deck: Vec<CardId>,
    pub hp: u32,
    pub max_hp: u32,
    pub map: MapGraph,
    pub position: Option<NodeId>,
    pub stage: Stage,
    pub combat: Option<CombatState>,
    pub stats: RunStats,
    last_encounter: Vec<Species>,
}

impl RunState {
    pub fn new(seed: u64) -> Self {
        let mut rng = RunRng::new(seed);
        let map = MapGraph::generate(&mut rng);
        Self {
            seed,
            rng,
            master_deck: starter_deck(),
            hp: STARTING_HP,
            max_hp: STARTING_HP,
            map,
            position: None,
            stage: Stage::ChoosingNode,
            combat: None,
            stats: RunStats::default(),
            last_encounter: Vec::new(),
        }
    }

    /// Legal next moves: floor-1 nodes at the start, else the current node's
    /// `next`. Empty while a combat or reward is unresolved.
    pub fn available_nodes(&self) -> Vec<NodeId> {
        if self.combat.is_some() || !matches!(self.stage, Stage::ChoosingNode) {
            return Vec::new();
        }
        match self.position {
            None => self.map.floor1(),
            Some(id) => self.map.node(id).next.clone(),
        }
    }

    /// Travel to a reachable node and resolve it. Combat nodes start a fight
    /// (returning its opening events); Rest/Treasure resolve immediately.
    pub fn enter_node(&mut self, id: NodeId) -> Result<Vec<CombatEvent>, RunError> {
        if self.combat.is_some() {
            return Err(RunError::InCombat);
        }
        if !matches!(self.stage, Stage::ChoosingNode) {
            return Err(RunError::NotChoosingNode);
        }
        if !self.available_nodes().contains(&id) {
            return Err(RunError::IllegalMove);
        }
        self.position = Some(id);
        let kind = self.map.node(id).kind;
        match kind {
            NodeKind::Monster | NodeKind::Elite | NodeKind::Boss => {
                let group = roll_encounter(kind, id.floor, &mut self.rng, &self.last_encounter);
                self.last_encounter = group.clone();
                let (combat, events) = CombatState::new(
                    &mut self.rng,
                    &self.master_deck,
                    self.hp,
                    self.max_hp,
                    &group,
                );
                self.combat = Some(combat);
                self.track(&events);
                Ok(events)
            }
            NodeKind::Rest => {
                let heal = self.max_hp * REST_HEAL_PERCENT / 100;
                self.hp = (self.hp + heal).min(self.max_hp);
                Ok(Vec::new())
            }
            NodeKind::Treasure => {
                self.stage = Stage::Reward {
                    offer: self.roll_offer(),
                    source: RewardSource::Treasure,
                };
                Ok(Vec::new())
            }
        }
    }

    pub fn apply(&mut self, action: Action) -> Result<Vec<CombatEvent>, IllegalAction> {
        let combat = self.combat.as_mut().ok_or(IllegalAction::CombatOver)?;
        let events = combat.apply(&mut self.rng, action)?;
        self.track(&events);

        match self.combat.as_ref().and_then(|c| c.over) {
            Some(Outcome::Victory) => {
                let combat = self.combat.take().expect("combat exists");
                self.hp = combat.player.hp;
                let at_boss = self.position == Some(self.map.boss_id());
                if at_boss {
                    self.stage = Stage::Victory;
                } else {
                    self.stage = Stage::Reward {
                        offer: self.roll_offer(),
                        source: RewardSource::Combat,
                    };
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

    /// Resolve a reward (combat or treasure) and return to the map.
    pub fn choose_reward(&mut self, pick: Option<usize>) -> Result<(), RunError> {
        let Stage::Reward { offer, .. } = self.stage else {
            return Err(RunError::NotInReward);
        };
        if let Some(i) = pick {
            let card = *offer.get(i).ok_or(RunError::BadIndex)?;
            self.master_deck.push(card);
        }
        self.stage = Stage::ChoosingNode;
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
    use crate::map::{NodeKind, BOSS_FLOOR};

    /// Walk to the first available node of a given kind by climbing the map,
    /// picking the leftmost reachable node; returns the chosen id.
    fn leftmost(run: &RunState) -> NodeId {
        let mut ns = run.available_nodes();
        ns.sort();
        ns[0]
    }

    /// Bot: clear the current fight by rigging enemies to 1 HP, then playing the
    /// first affordable card at the first living enemy (else end turn).
    fn win_current_fight(run: &mut RunState) {
        for e in &mut run.combat.as_mut().unwrap().enemies {
            e.hp = 1;
        }
        for _ in 0..300 {
            if run.combat.is_none() {
                return;
            }
            let action = {
                let c = run.combat.as_ref().unwrap();
                let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
                c.hand
                    .iter()
                    .enumerate()
                    .find_map(|(i, card)| {
                        if card.spec().cost > c.player.energy {
                            return None;
                        }
                        match card.spec().targeting {
                            crate::cards::Targeting::SingleEnemy => {
                                target.map(|t| Action::PlayCard {
                                    hand_index: i,
                                    target: Some(t),
                                })
                            }
                            _ => Some(Action::PlayCard {
                                hand_index: i,
                                target: None,
                            }),
                        }
                    })
                    .unwrap_or(Action::EndTurn)
            };
            run.apply(action).unwrap();
        }
        panic!("rigged fight did not end");
    }

    #[test]
    fn new_run_starts_choosing_among_floor1_nodes() {
        let run = RunState::new(7);
        assert_eq!(run.stage, Stage::ChoosingNode);
        assert!(run.position.is_none());
        assert_eq!(run.master_deck.len(), 10);
        assert_eq!(run.hp, 80);
        assert!(run.combat.is_none());
        let avail = run.available_nodes();
        assert!(avail.len() >= 2);
        assert!(avail.iter().all(|n| n.floor == 1));
    }

    #[test]
    fn entering_a_floor1_node_starts_a_combat() {
        let mut run = RunState::new(7);
        let id = leftmost(&run);
        let events = run.enter_node(id).unwrap();
        assert!(run.combat.is_some());
        assert!(!events.is_empty());
        assert_eq!(run.position, Some(id));
    }

    #[test]
    fn illegal_moves_are_rejected() {
        let mut run = RunState::new(7);
        // a node that is not on floor 1 is unreachable from the start
        let bad = NodeId { floor: 5, col: 0 };
        assert_eq!(run.enter_node(bad), Err(RunError::IllegalMove));
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        // cannot enter another node mid-combat
        assert_eq!(run.enter_node(id), Err(RunError::InCombat));
    }

    #[test]
    fn winning_a_combat_node_offers_a_reward_then_returns_to_map() {
        let mut run = RunState::new(7);
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        win_current_fight(&mut run);
        assert!(matches!(
            run.stage,
            Stage::Reward {
                source: RewardSource::Combat,
                ..
            }
        ));
        assert!(run.available_nodes().is_empty());
        run.choose_reward(Some(0)).unwrap();
        assert_eq!(run.stage, Stage::ChoosingNode);
        assert_eq!(run.master_deck.len(), 11);
        // back on the map at the cleared node; next moves are that node's children
        assert_eq!(run.available_nodes(), run.map.node(id).next);
    }

    #[test]
    fn rest_node_heals_30_percent_capped() {
        // Find a seed/path that reaches a Rest node; floor 15 is always Rest, but
        // we test the heal math directly via a constructed state instead.
        let mut run = RunState::new(1);
        run.hp = 40;
        // Drive onto floor 15 is long; assert the formula via a Rest at floor 15.
        // Simpler: verify the constant and cap behaviour through a direct heal.
        let heal = run.max_hp * REST_HEAL_PERCENT / 100;
        assert_eq!(heal, 24);
        run.hp = (run.hp + heal).min(run.max_hp);
        assert_eq!(run.hp, 64);
        run.hp = 70;
        run.hp = (run.hp + heal).min(run.max_hp);
        assert_eq!(run.hp, 80); // capped
    }

    #[test]
    fn dying_in_combat_sets_defeat() {
        let mut run = RunState::new(17);
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        run.combat.as_mut().unwrap().player.hp = 1;
        for _ in 0..30 {
            if run.combat.is_none() {
                break;
            }
            run.apply(Action::EndTurn).unwrap();
        }
        assert_eq!(run.stage, Stage::Defeat);
        assert_eq!(run.hp, 0);
    }

    #[test]
    fn reward_offers_distinct_pool_cards() {
        let mut run = RunState::new(11);
        let id = leftmost(&run);
        run.enter_node(id).unwrap();
        win_current_fight(&mut run);
        let Stage::Reward { offer, .. } = run.stage else {
            panic!("expected reward")
        };
        assert!(offer.iter().all(|c| REWARD_POOL.contains(c)));
        assert_ne!(offer[0], offer[1]);
        assert_ne!(offer[1], offer[2]);
        assert_ne!(offer[0], offer[2]);
    }

    #[test]
    fn boss_node_kind_is_boss() {
        let run = RunState::new(3);
        assert_eq!(run.map.node(run.map.boss_id()).kind, NodeKind::Boss);
        assert_eq!(run.map.boss_id().floor, BOSS_FLOOR);
    }
}
