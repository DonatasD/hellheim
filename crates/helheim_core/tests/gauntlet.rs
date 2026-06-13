use helheim_core::cards::Targeting;
use helheim_core::combat::{Action, CombatState};
use helheim_core::run::{RunState, Stage};

/// Dumb-but-legal policy: play the first affordable card (targeting the
/// first living enemy when needed), else end the turn.
fn choose_action(c: &CombatState) -> Action {
    let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
    for (i, card) in c.hand.iter().enumerate() {
        let spec = card.spec();
        if spec.cost > c.player.energy {
            continue;
        }
        return match spec.targeting {
            Targeting::SingleEnemy => match target {
                Some(t) => Action::PlayCard {
                    hand_index: i,
                    target: Some(t),
                },
                None => continue,
            },
            _ => Action::PlayCard {
                hand_index: i,
                target: None,
            },
        };
    }
    Action::EndTurn
}

/// Drive a whole run with the policy bot. Returns the finished run and the
/// debug-formatted event log (the determinism fingerprint).
fn run_bot(seed: u64) -> (RunState, Vec<String>) {
    let mut run = RunState::new(seed);
    let mut log = Vec::new();
    for _ in 0..10_000 {
        match run.stage {
            Stage::Fight(_) => {
                if run.combat.is_none() {
                    for e in run.begin_fight().unwrap() {
                        log.push(format!("{e:?}"));
                    }
                } else {
                    let action = choose_action(run.combat.as_ref().unwrap());
                    for e in run.apply(action).unwrap() {
                        log.push(format!("{e:?}"));
                    }
                }
            }
            Stage::Reward { .. } => run.choose_reward(Some(0)).unwrap(),
            Stage::Victory | Stage::Defeat => return (run, log),
        }
    }
    panic!("bot did not finish a run in 10k steps (seed {seed})");
}

#[test]
fn every_seed_reaches_a_terminal_stage_with_consistent_state() {
    for seed in 0..25u64 {
        let (run, log) = run_bot(seed);
        assert!(!log.is_empty());
        match run.stage {
            Stage::Victory => {
                assert!(run.hp > 0, "seed {seed}: won with 0 hp");
                assert_eq!(run.master_deck.len(), 12, "two rewards were picked");
                assert!(run.stats.damage_dealt > 0);
            }
            Stage::Defeat => assert_eq!(run.hp, 0, "seed {seed}: lost with hp left"),
            other => panic!("seed {seed}: non-terminal stage {other:?}"),
        }
        assert!(run.stats.turns > 0);
        assert!(run.combat.is_none());
    }
}

#[test]
fn same_seed_same_run_byte_for_byte() {
    let (run_a, log_a) = run_bot(424242);
    let (run_b, log_b) = run_bot(424242);
    assert_eq!(log_a, log_b);
    assert_eq!(run_a.stage, run_b.stage);
    assert_eq!(run_a.hp, run_b.hp);
    assert_eq!(run_a.master_deck, run_b.master_deck);
    assert_eq!(run_a.stats, run_b.stats);
}

#[test]
fn different_seeds_diverge() {
    let (_, log_a) = run_bot(1);
    let (_, log_b) = run_bot(2);
    assert_ne!(log_a, log_b);
}
