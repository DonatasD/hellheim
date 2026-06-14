use std::collections::{BTreeMap, BTreeSet};

use crate::rng::RunRng;

pub const MAP_FLOORS: u8 = 15;
pub const MAP_WIDTH: u8 = 7;
pub const MAP_PATHS: u32 = 6;
pub const BOSS_FLOOR: u8 = 16;
pub const BOSS_COL: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum NodeKind {
    Monster,
    Elite,
    Rest,
    Treasure,
    Boss,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct NodeId {
    pub floor: u8,
    pub col: u8,
}

#[derive(Clone, Debug)]
pub struct MapNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub next: Vec<NodeId>,
}

#[derive(Clone, Debug)]
pub struct MapGraph {
    nodes: Vec<MapNode>, // floor-ascending; boss last
}

/// Two edges between the same pair of floors cross when their endpoints invert.
fn crosses(edges: &[(u8, u8)], a: u8, b: u8) -> bool {
    edges
        .iter()
        .any(|&(a2, b2)| (a < a2 && b > b2) || (a > a2 && b < b2))
}

/// Build `MAP_PATHS` column-per-floor walks bottom→top, adjacency-stepped and
/// crossing-free (straight-up is always safe, so a legal step always exists).
fn build_paths(rng: &mut RunRng) -> Vec<Vec<u8>> {
    let mut starts: Vec<u8> = (0..MAP_PATHS)
        .map(|_| rng.range(0, (MAP_WIDTH - 1) as u32) as u8)
        .collect();
    if starts.iter().collect::<BTreeSet<_>>().len() < 2 {
        starts[1] = (starts[0] + 1) % MAP_WIDTH;
    }

    let mut edges: Vec<Vec<(u8, u8)>> = vec![Vec::new(); (MAP_FLOORS - 1) as usize];
    let mut paths = Vec::new();
    for start in starts {
        let mut path = vec![start];
        for f in 0..(MAP_FLOORS - 1) as usize {
            let a = path[f] as i32;
            let mut cands: Vec<u8> = [a - 1, a, a + 1]
                .into_iter()
                .filter(|&c| (0..MAP_WIDTH as i32).contains(&c))
                .map(|c| c as u8)
                .filter(|&b| !crosses(&edges[f], path[f], b))
                .collect();
            if cands.is_empty() {
                cands.push(path[f]); // straight up: never crosses
            }
            let b = cands[rng.range(0, (cands.len() - 1) as u32) as usize];
            edges[f].push((path[f], b));
            path.push(b);
        }
        paths.push(path);
    }
    paths
}

impl MapGraph {
    pub fn generate(rng: &mut RunRng) -> Self {
        let paths = build_paths(rng);

        let mut cols_on: Vec<BTreeSet<u8>> = vec![BTreeSet::new(); MAP_FLOORS as usize];
        let mut next_of: BTreeMap<NodeId, BTreeSet<NodeId>> = BTreeMap::new();
        let boss = NodeId {
            floor: BOSS_FLOOR,
            col: BOSS_COL,
        };

        for path in &paths {
            for (f, &col) in path.iter().enumerate() {
                cols_on[f].insert(col);
            }
            for f in 0..(MAP_FLOORS - 1) as usize {
                let from = NodeId {
                    floor: f as u8 + 1,
                    col: path[f],
                };
                let to = NodeId {
                    floor: f as u8 + 2,
                    col: path[f + 1],
                };
                next_of.entry(from).or_default().insert(to);
            }
        }
        for &col in &cols_on[(MAP_FLOORS - 1) as usize] {
            next_of
                .entry(NodeId {
                    floor: MAP_FLOORS,
                    col,
                })
                .or_default()
                .insert(boss);
        }

        let mut nodes = Vec::new();
        for (f, cols) in cols_on.iter().enumerate() {
            for &col in cols {
                let id = NodeId {
                    floor: f as u8 + 1,
                    col,
                };
                let next = next_of
                    .get(&id)
                    .map(|s| s.iter().copied().collect())
                    .unwrap_or_default();
                nodes.push(MapNode {
                    id,
                    kind: NodeKind::Monster,
                    next,
                });
            }
        }
        nodes.push(MapNode {
            id: boss,
            kind: NodeKind::Boss,
            next: Vec::new(),
        });

        MapGraph { nodes }
        // Kinds (except the boss) are assigned in Task 5.
    }

    pub fn floor1(&self) -> Vec<NodeId> {
        self.nodes_on(1).iter().map(|n| n.id).collect()
    }

    pub fn node(&self, id: NodeId) -> &MapNode {
        self.nodes.iter().find(|n| n.id == id).expect("node exists")
    }

    pub fn nodes_on(&self, floor: u8) -> Vec<&MapNode> {
        self.nodes.iter().filter(|n| n.id.floor == floor).collect()
    }

    pub fn boss_id(&self) -> NodeId {
        NodeId {
            floor: BOSS_FLOOR,
            col: BOSS_COL,
        }
    }

    pub fn all(&self) -> &[MapNode] {
        &self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::RunRng;
    use std::collections::{HashSet, VecDeque};

    fn gen(seed: u64) -> MapGraph {
        MapGraph::generate(&mut RunRng::new(seed))
    }

    #[test]
    fn has_15_floors_plus_a_boss() {
        let g = gen(1);
        for f in 1..=MAP_FLOORS {
            assert!(!g.nodes_on(f).is_empty(), "floor {f} empty");
        }
        assert_eq!(
            g.boss_id(),
            NodeId {
                floor: BOSS_FLOOR,
                col: BOSS_COL
            }
        );
        assert_eq!(g.nodes_on(BOSS_FLOOR).len(), 1);
    }

    #[test]
    fn at_least_two_distinct_starts() {
        for seed in 0..50 {
            assert!(gen(seed).floor1().len() >= 2, "seed {seed}");
        }
    }

    #[test]
    fn edges_step_one_floor_to_adjacent_columns() {
        for seed in 0..50 {
            let g = gen(seed);
            for n in g.all() {
                for nx in &n.next {
                    assert_eq!(nx.floor, n.id.floor + 1, "seed {seed}: non-adjacent floor");
                    if nx.floor <= MAP_FLOORS {
                        let d = (nx.col as i32 - n.id.col as i32).abs();
                        assert!(d <= 1, "seed {seed}: column jump {d}");
                    }
                }
            }
        }
    }

    #[test]
    fn no_crossing_edges() {
        for seed in 0..50 {
            let g = gen(seed);
            for f in 1..MAP_FLOORS {
                let mut edges: Vec<(u8, u8)> = Vec::new();
                for n in g.nodes_on(f) {
                    for nx in &n.next {
                        edges.push((n.id.col, nx.col));
                    }
                }
                for (i, &(a, b)) in edges.iter().enumerate() {
                    for &(a2, b2) in &edges[i + 1..] {
                        let cross = (a < a2 && b > b2) || (a > a2 && b < b2);
                        assert!(!cross, "seed {seed} floor {f}: edges cross");
                    }
                }
            }
        }
    }

    #[test]
    fn every_start_reaches_the_boss_no_orphans() {
        for seed in 0..50 {
            let g = gen(seed);
            // BFS from floor-1 nodes; every node must be reachable, and the boss reached.
            let mut seen: HashSet<NodeId> = HashSet::new();
            let mut q: VecDeque<NodeId> = g.floor1().into_iter().collect();
            for id in &q {
                seen.insert(*id);
            }
            while let Some(id) = q.pop_front() {
                for nx in &g.node(id).next {
                    if seen.insert(*nx) {
                        q.push_back(*nx);
                    }
                }
            }
            assert!(seen.contains(&g.boss_id()), "seed {seed}: boss unreachable");
            for n in g.all() {
                assert!(seen.contains(&n.id), "seed {seed}: orphan {:?}", n.id);
            }
        }
    }

    #[test]
    fn generation_is_deterministic() {
        assert_eq!(
            format!("{:?}", gen(123).all()),
            format!("{:?}", gen(123).all())
        );
    }
}
