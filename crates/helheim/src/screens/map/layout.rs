//! Pure map geometry & visual classification — no Bevy ECS, fully unit-tested.
use bevy::prelude::*;
use helheim_core::map::{NodeId, NodeKind, BOSS_FLOOR, MAP_WIDTH};
use crate::theme;

/// Vertical world units between floors.
pub const FLOOR_GAP: f32 = 120.0;
/// Horizontal world units between columns.
pub const COL_GAP: f32 = 110.0;
/// Node hit/visual radius in world units.
pub const NODE_R: f32 = 30.0;
/// Camera-follow clamp: enough offset to frame floor 1 without overscrolling.
pub const MIN_CAM_Y: f32 = 2.0 * FLOOR_GAP;
/// Upper camera clamp: stops one floor short of the boss so the boss frames near the top.
pub const MAX_CAM_Y: f32 = (BOSS_FLOOR as f32 - 1.0) * FLOOR_GAP;

/// World position of a node: column centered horizontally, floor stacked upward.
pub fn node_pos(id: NodeId) -> Vec2 {
    let x = (id.col as f32 - (MAP_WIDTH as f32 - 1.0) / 2.0) * COL_GAP;
    let y = id.floor as f32 * FLOOR_GAP;
    Vec2::new(x, y)
}

fn cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

/// Point at fraction `t` along the vertical-tangent cubic between two nodes.
pub fn bezier_point_at(from: Vec2, to: Vec2, t: f32) -> Vec2 {
    let my = (from.y + to.y) * 0.5;
    cubic(from, Vec2::new(from.x, my), Vec2::new(to.x, my), to, t)
}

/// `n + 1` samples along the curve (endpoints included).
pub fn bezier_points(from: Vec2, to: Vec2, n: usize) -> Vec<Vec2> {
    assert!(n >= 1, "bezier_points: n must be at least 1");
    (0..=n).map(|i| bezier_point_at(from, to, i as f32 / n as f32)).collect()
}

/// Camera-follow target for a floor, clamped so the view never overscrolls.
pub fn camera_y_for(floor: u8) -> f32 {
    (floor as f32 * FLOOR_GAP).clamp(MIN_CAM_Y, MAX_CAM_Y)
}

/// Nearest node whose center is within `NODE_R` of a world-space cursor point.
pub fn pick_node(cursor: Vec2, nodes: &[(NodeId, Vec2)]) -> Option<NodeId> {
    nodes
        .iter()
        .filter(|(_, p)| p.distance(cursor) <= NODE_R)
        .min_by(|a, b| {
            a.1.distance(cursor)
                .partial_cmp(&b.1.distance(cursor))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, _)| *id)
}

/// Step the keyboard selection across the reachable set (sorted by `(floor, col)`),
/// wrapping at both ends. `dir` is +1 (right) or -1 (left).
pub fn keyboard_step(current: NodeId, reachable: &[NodeId], dir: i8) -> NodeId {
    let mut sorted = reachable.to_vec();
    sorted.sort_by_key(|n| (n.floor, n.col));
    if sorted.is_empty() {
        return current;
    }
    // `current` absent from the set → step from index 0.
    let idx = sorted.iter().position(|&n| n == current).unwrap_or(0) as i32;
    let len = sorted.len() as i32;
    sorted[(idx + dir as i32).rem_euclid(len) as usize]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeVisual {
    Current,
    Reachable,
    Visited,
    Locked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeStyle {
    Taken,     // road through already-climbed floors
    Available, // leaves the current node to a reachable child
    Ahead,     // unreached future road
}

/// Per-kind accent color for rings, icon tint, and glow.
pub fn kind_color(kind: NodeKind) -> Color {
    match kind {
        NodeKind::Monster => Color::srgb(0.55, 0.56, 0.60), // steel
        NodeKind::Elite => theme::ACCENT,                   // red
        NodeKind::Rest => Color::srgb(0.95, 0.64, 0.23),    // warm orange
        NodeKind::Treasure => theme::ENERGY_COLOR,          // gold
        NodeKind::Boss => Color::srgb(0.89, 0.70, 0.29),    // bright gold
    }
}

/// Classify a node for rendering, given where the player stands and what's reachable.
pub fn node_visual(id: NodeId, current: Option<NodeId>, reachable: &[NodeId]) -> NodeVisual {
    if Some(id) == current {
        NodeVisual::Current
    } else if reachable.contains(&id) {
        NodeVisual::Reachable
    } else if id.floor < current.map(|c| c.floor).unwrap_or(0) {
        NodeVisual::Visited
    } else {
        NodeVisual::Locked
    }
}

/// Classify an edge `from → to` for rendering.
pub fn edge_style(
    from: NodeId,
    to: NodeId,
    current: Option<NodeId>,
    reachable: &[NodeId],
) -> EdgeStyle {
    if Some(from) == current && reachable.contains(&to) {
        EdgeStyle::Available
    } else if to.floor <= current.map(|c| c.floor).unwrap_or(0) {
        EdgeStyle::Taken
    } else {
        EdgeStyle::Ahead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_pos_climbs_and_centers() {
        assert!(node_pos(NodeId { floor: 2, col: 0 }).y > node_pos(NodeId { floor: 1, col: 0 }).y);
        assert!(node_pos(NodeId { floor: 1, col: 6 }).x > node_pos(NodeId { floor: 1, col: 0 }).x);
        // MAP_WIDTH == 7, so the middle column (3) sits on the x axis.
        assert!(node_pos(NodeId { floor: 1, col: 3 }).x.abs() < 1e-3);
        assert!((node_pos(NodeId { floor: 0, col: 0 }).x + 3.0 * COL_GAP).abs() < 1e-3);
    }

    #[test]
    fn bezier_hits_its_endpoints() {
        let a = Vec2::new(0., 0.);
        let b = Vec2::new(110., 120.);
        let pts = bezier_points(a, b, 8);
        assert_eq!(pts.len(), 9);
        assert!((pts[0] - a).length() < 1e-3);
        assert!((pts.last().unwrap() - b).length() < 1e-3);
    }

    #[test]
    fn camera_clamps_at_both_ends_and_climbs_between() {
        assert_eq!(camera_y_for(1), MIN_CAM_Y); // below the frame floor → clamped up
        assert_eq!(camera_y_for(BOSS_FLOOR), MAX_CAM_Y);
        assert!(camera_y_for(8) > camera_y_for(4));
    }

    #[test]
    fn pick_node_returns_nearest_within_radius() {
        let nodes = vec![
            (NodeId { floor: 1, col: 0 }, Vec2::new(0., 0.)),    // ~7 from cursor
            (NodeId { floor: 1, col: 2 }, Vec2::new(20., 0.)),   // ~20 from cursor, also within NODE_R
            (NodeId { floor: 1, col: 1 }, Vec2::new(200., 0.)),  // far outside
        ];
        // Both col:0 and col:2 are within NODE_R; the nearer (col:0) must win.
        assert_eq!(pick_node(Vec2::new(5., 5.), &nodes), Some(NodeId { floor: 1, col: 0 }));
        assert_eq!(pick_node(Vec2::new(500., 500.), &nodes), None);
    }

    #[test]
    fn keyboard_step_cycles_and_wraps() {
        let r = vec![
            NodeId { floor: 2, col: 1 },
            NodeId { floor: 2, col: 3 },
            NodeId { floor: 2, col: 5 },
        ];
        assert_eq!(keyboard_step(r[0], &r, 1), r[1]);
        assert_eq!(keyboard_step(r[2], &r, 1), r[0]); // wrap forward
        assert_eq!(keyboard_step(r[0], &r, -1), r[2]); // wrap backward
    }

    #[test]
    fn kind_color_is_distinct_and_themed() {
        assert_eq!(kind_color(NodeKind::Elite), theme::ACCENT);
        assert_ne!(kind_color(NodeKind::Monster), kind_color(NodeKind::Rest));
    }

    #[test]
    fn node_visual_classifies_by_position_and_reach() {
        let cur = NodeId { floor: 3, col: 2 };
        let reach = vec![NodeId { floor: 4, col: 1 }, NodeId { floor: 4, col: 2 }];
        assert_eq!(node_visual(cur, Some(cur), &reach), NodeVisual::Current);
        assert_eq!(node_visual(reach[0], Some(cur), &reach), NodeVisual::Reachable);
        assert_eq!(node_visual(NodeId { floor: 1, col: 0 }, Some(cur), &reach), NodeVisual::Visited);
        assert_eq!(node_visual(NodeId { floor: 6, col: 0 }, Some(cur), &reach), NodeVisual::Locked);
    }

    #[test]
    fn edge_style_classifies_taken_available_ahead() {
        let cur = NodeId { floor: 3, col: 2 };
        let reach = vec![NodeId { floor: 4, col: 2 }];
        assert_eq!(edge_style(cur, reach[0], Some(cur), &reach), EdgeStyle::Available);
        assert_eq!(
            edge_style(NodeId { floor: 1, col: 0 }, NodeId { floor: 2, col: 0 }, Some(cur), &reach),
            EdgeStyle::Taken
        );
        assert_eq!(
            edge_style(NodeId { floor: 5, col: 0 }, NodeId { floor: 6, col: 0 }, Some(cur), &reach),
            EdgeStyle::Ahead
        );
    }
}
