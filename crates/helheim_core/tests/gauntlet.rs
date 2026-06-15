use helheim_core::cards::Targeting;
use helheim_core::combat::{Action, CombatState};
use helheim_core::map::NodeId;
use helheim_core::run::{RunState, Stage};

fn choose_action(c: &CombatState) -> Action {
    let target = (0..c.enemies.len()).find(|&i| c.enemies[i].alive());
    for (i, card) in c.hand.iter().enumerate() {
        if card.spec().cost > c.player.energy {
            continue;
        }
        return match card.spec().targeting {
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

/// Deterministic map policy: always travel to the leftmost reachable node.
fn leftmost(run: &RunState) -> NodeId {
    let mut ns = run.available_nodes();
    ns.sort();
    assert!(!ns.is_empty(), "no available nodes from {:?}", run.position);
    ns[0]
}

fn run_bot(seed: u64) -> (RunState, Vec<String>) {
    let mut run = RunState::new(seed);
    let mut log = Vec::new();
    for _ in 0..100_000 {
        if let Some(c) = run.combat.as_ref() {
            let action = choose_action(c);
            for e in run.apply(action).unwrap() {
                log.push(format!("{e:?}"));
            }
            continue;
        }
        match run.stage {
            Stage::ChoosingNode => {
                let id = leftmost(&run);
                log.push(format!("Enter {id:?}"));
                for e in run.enter_node(id).unwrap() {
                    log.push(format!("{e:?}"));
                }
            }
            Stage::Reward { .. } => run.choose_reward(Some(0)).unwrap(),
            Stage::Victory | Stage::Defeat => return (run, log),
        }
    }
    panic!("bot did not finish a run in 100k steps (seed {seed})");
}

#[test]
fn every_seed_reaches_a_terminal_stage() {
    for seed in 0..25u64 {
        let (run, log) = run_bot(seed);
        assert!(!log.is_empty());
        match run.stage {
            Stage::Victory => {
                assert!(run.hp > 0, "seed {seed}: won at 0 hp");
                // Reaching the boss means clearing intermediary combat/treasure
                // nodes, each of which grants a card the bot takes (10 starter +
                // ≥1 reward along any leftmost path to the boss).
                assert!(
                    run.master_deck.len() >= 11,
                    "seed {seed}: deck did not grow from rewards ({})",
                    run.master_deck.len()
                );
            }
            Stage::Defeat => assert_eq!(run.hp, 0, "seed {seed}: lost with hp"),
            other => panic!("seed {seed}: non-terminal {other:?}"),
        }
        assert!(run.stats.turns > 0);
        assert!(run.combat.is_none());
    }
}

#[test]
fn same_seed_same_run_byte_for_byte() {
    let (a, la) = run_bot(424242);
    let (b, lb) = run_bot(424242);
    assert_eq!(la, lb);
    assert_eq!(a.stage, b.stage);
    assert_eq!(a.hp, b.hp);
    assert_eq!(a.master_deck, b.master_deck);
    assert_eq!(a.stats, b.stats);
}

#[test]
fn different_seeds_diverge() {
    let (_, la) = run_bot(1);
    let (_, lb) = run_bot(2);
    assert_ne!(la, lb);
}
