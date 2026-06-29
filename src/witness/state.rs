use crate::solver::{SearchState, UndoStack};
use crate::witness::graph::WitnessGraph;
use crate::witness::types::{EdgeId, NodeId};

// ---------------------------------------------------------------------------
// Bitset helpers for used_edges
// ---------------------------------------------------------------------------

#[inline(always)]
pub fn test_bit(bits: &[u64], i: usize) -> bool {
    let w = i >> 6;
    let b = i & 63;
    ((bits[w] >> b) & 1) != 0
}

#[inline(always)]
fn set_bit(bits: &mut [u64], i: usize) {
    let w = i >> 6;
    let b = i & 63;
    bits[w] |= 1u64 << b;
}

#[inline(always)]
fn clear_bit(bits: &mut [u64], i: usize) {
    let w = i >> 6;
    let b = i & 63;
    bits[w] &= !(1u64 << b);
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct WitnessState {
    pub used_edges: Vec<u64>,
    pub degrees: Vec<u8>,
    pub head: NodeId,
}

pub enum UndoEntry {
    ClearEdgeBit { edge_index: EdgeId },
    DecDeg { node_index: NodeId },
    Head { prev: NodeId },
}

impl WitnessState {
    pub fn new(g: &WitnessGraph) -> Self {
        let num_slots = g.num_edge_slots();
        let used_edges = vec![0u64; num_slots.div_ceil(64)];
        let degrees = vec![0u8; g.num_nodes()];
        WitnessState {
            used_edges,
            degrees,
            head: g.start,
        }
    }

    #[inline]
    pub fn used(&self, ei: usize) -> bool {
        test_bit(&self.used_edges, ei)
    }

    /// Return true if `ni` is a valid path terminus (can have degree 1).
    /// For symmetry: both player's end and mirror's end (and mirror's start) count.
    pub fn is_end_node(&self, graph: &WitnessGraph, ni: NodeId) -> bool {
        if ni == graph.end {
            return true;
        }
        if graph.symmetry.is_some() {
            if let Some(me) = graph.symmetric_node(graph.end)
                && ni == me
            {
                return true;
            }
            if let Some(ms) = graph.symmetric_node(graph.start)
                && ni == ms
            {
                return true;
            }
        }
        false
    }
}

impl SearchState for WitnessState {
    type Move = EdgeId; // edge index
    type UndoEntry = UndoEntry;
    type Ctx = WitnessGraph;

    fn gen_moves(&self, ctx: &WitnessGraph, out: &mut Vec<Self::Move>) {
        if self.head == ctx.end {
            return;
        }
        let head = self.head;
        let moves_start = out.len();
        ctx.for_each_neighbor(head, |v| {
            let ei = ctx.edge_endpoints_to_idx(head, v);

            if self.used(ei) || ctx.is_broken(ei) {
                return;
            }
            if self.degrees[v] != 0 && !self.is_end_node(ctx, v) {
                return;
            }

            if ctx.symmetry.is_some()
                && let Some(me) = ctx.symmetric_edge(ei)
            {
                if self.used(me) || ctx.is_broken(me) {
                    return;
                }
                let (m_u, m_v) = ctx.edge_idx_to_endpoints(me);
                let mirror_head = ctx.symmetric_node(head).unwrap_or(head);
                let mirror_target = if m_u == mirror_head { m_v } else { m_u };
                if self.degrees[mirror_target] != 0 && !self.is_end_node(ctx, mirror_target) {
                    return;
                }
            }

            out.push(ei);
        });

        let end_xy = ctx.node_idx_to_xy(ctx.end);
        out[moves_start..].sort_by_key(|&ei| {
            let (u, v) = ctx.edge_idx_to_endpoints(ei);
            let target = if u == head { v } else { u };
            let target_xy = ctx.node_idx_to_xy(target);
            let dot_priority = if ctx.dot_edges.contains(&ei)
                || ctx
                    .colored_dot_edges
                    .iter()
                    .any(|&(dot_ei, _)| dot_ei == ei)
                || ctx.dot_nodes.contains(&target)
                || ctx
                    .colored_dot_nodes
                    .iter()
                    .any(|&(dot_node, _)| dot_node == target)
            {
                0usize
            } else {
                1usize
            };
            let end_distance = target_xy.0.abs_diff(end_xy.0) + target_xy.1.abs_diff(end_xy.1);
            (dot_priority, end_distance, ei)
        });
    }

    fn apply_move(&mut self, ctx: &WitnessGraph, mv: Self::Move, undo: &mut UndoStack<Self>) {
        undo.mark();

        let (u, v) = ctx.edge_idx_to_endpoints(mv);

        debug_assert!(!test_bit(&self.used_edges, mv));
        undo.push(UndoEntry::ClearEdgeBit { edge_index: mv });
        set_bit(&mut self.used_edges, mv);

        self.degrees[u] += 1;
        undo.push(UndoEntry::DecDeg { node_index: u });
        self.degrees[v] += 1;
        undo.push(UndoEntry::DecDeg { node_index: v });

        if let Some(me) = ctx.symmetric_edge(mv)
            && me != mv
        {
            debug_assert!(!test_bit(&self.used_edges, me));
            undo.push(UndoEntry::ClearEdgeBit { edge_index: me });
            set_bit(&mut self.used_edges, me);

            let (m_u, m_v) = ctx.edge_idx_to_endpoints(me);
            if m_u != u && m_u != v {
                self.degrees[m_u] += 1;
                undo.push(UndoEntry::DecDeg { node_index: m_u });
            }
            if m_v != u && m_v != v {
                self.degrees[m_v] += 1;
                undo.push(UndoEntry::DecDeg { node_index: m_v });
            }
        }

        debug_assert!(
            self.head == u || self.head == v,
            "head={}, u={}, v={}, mv={}",
            self.head,
            u,
            v,
            mv
        );
        undo.push(UndoEntry::Head { prev: self.head });
        self.head = if self.head == u { v } else { u };
    }

    fn apply_undo(&mut self, entry: Self::UndoEntry) {
        match entry {
            UndoEntry::ClearEdgeBit { edge_index } => clear_bit(&mut self.used_edges, edge_index),
            UndoEntry::DecDeg { node_index } => self.degrees[node_index] -= 1,
            UndoEntry::Head { prev } => self.head = prev,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::{Satisfier, UndoStack};
    use crate::witness::graph::{PuzzleJson, SymmetryKind, WitnessGraph};
    use crate::witness::rules::WitnessValidator;

    fn make_symmetry_graph() -> WitnessGraph {
        let json = PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[4, 2]],
            symmetry: Some(SymmetryKind::MirrorX),
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
        };
        WitnessGraph::from_json(json).unwrap()
    }

    #[test]
    fn self_symmetric_edge_counts_once() {
        let graph = make_symmetry_graph();
        let mv = graph.v_edge_index(2, 0);
        assert!(graph.symmetric_edge(mv).is_none());
        let mut state = WitnessState::new(&graph);
        state.head = graph.node_xy_to_idx(2, 0);
        state.degrees[graph.start] = 0;
        state.degrees[state.head] = 0;
        let mut undo = UndoStack::new();
        state.apply_move(&graph, mv, &mut undo);
        let (u, v) = graph.edge_idx_to_endpoints(mv);
        assert!(state.used(mv));
        assert_eq!(state.degrees[u], 1);
        assert_eq!(state.degrees[v], 1);
    }

    #[test]
    fn axis_collision_degree_is_two() {
        let graph = make_symmetry_graph();
        let mut state = WitnessState::new(&graph);
        let mut undo = UndoStack::new();

        state.apply_move(&graph, graph.h_edge_index(0, 0), &mut undo);
        state.apply_move(&graph, graph.h_edge_index(1, 0), &mut undo);
        state.apply_move(&graph, graph.v_edge_index(2, 0), &mut undo);

        let axis_node = graph.node_xy_to_idx(2, 0);
        assert_eq!(state.degrees[axis_node], 2);
    }

    #[test]
    fn is_end_node_recognizes_mirror_end() {
        let graph = make_symmetry_graph();
        let state = WitnessState::new(&graph);
        assert!(state.is_end_node(&graph, graph.end));
        let mirror_end = graph.symmetric_node(graph.end).unwrap();
        assert!(state.is_end_node(&graph, mirror_end));
        let mirror_start = graph.symmetric_node(graph.start).unwrap();
        assert!(state.is_end_node(&graph, mirror_start));
        let mid = graph.node_xy_to_idx(2, 1);
        assert!(!state.is_end_node(&graph, mid));
    }

    #[test]
    fn broken_edge_blocks_both_player_and_mirror() {
        let json = PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[4, 2]],
            symmetry: Some(SymmetryKind::MirrorX),
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![[[0, 0], [1, 0]]],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
        };
        let graph = WitnessGraph::from_json(json).unwrap();
        let state = WitnessState::new(&graph);
        let player_edge = graph.h_edge_index(0, 0);
        assert!(graph.is_broken(player_edge));
        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);
        assert!(!moves.contains(&player_edge));
    }

    // -----------------------------------------------------------------------
    // Non-symmetry: gen_moves, apply/undo, is_satisfied
    // -----------------------------------------------------------------------

    /// Build a simple 2×2 non-symmetry puzzle (start at (0,0), end at (2,2)).
    fn make_graph() -> WitnessGraph {
        let json = PuzzleJson {
            width: 2,
            height: 2,
            starts: vec![[0, 0]],
            ends: vec![[2, 2]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
        };
        WitnessGraph::from_json(json).unwrap()
    }

    #[test]
    fn gen_moves_initial() {
        let graph = make_graph();
        // Start at (0,0)=node 0. Neighbors: right=node 1 (h_edge(0,0)=0),
        // down=node 3 (v_edge(0,0)=1). Both should be generated.
        let state = WitnessState::new(&graph);
        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);
        moves.sort();
        assert_eq!(moves, vec![0, 1]);
    }

    #[test]
    fn gen_moves_broken_edge_excluded() {
        // Break h_edge(0,0) — the rightward move from start.
        let json = PuzzleJson {
            width: 2,
            height: 2,
            starts: vec![[0, 0]],
            ends: vec![[2, 2]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![[[0, 0], [1, 0]]],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
        };
        let graph = WitnessGraph::from_json(json).unwrap();
        let state = WitnessState::new(&graph);
        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);
        // Only v_edge(0,0)=1 (down) should be generated
        assert_eq!(moves, vec![1]);
    }

    #[test]
    fn gen_moves_used_edge_excluded() {
        let graph = make_graph();
        let mut state = WitnessState::new(&graph);
        let mut undo = UndoStack::new();

        // Move right: (0,0) → (1,0) via h_edge(0,0)=0
        state.apply_move(&graph, graph.h_edge_index(0, 0), &mut undo);

        // Head is now at node 1. h_edge(0,0) is used, so it must NOT appear.
        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);

        assert!(!moves.contains(&0), "used edge 0 should be excluded");
        assert!(moves.contains(&2), "h_edge(1,0)=2 should be generated");
        assert!(moves.contains(&3), "v_edge(1,0)=3 should be generated");
    }

    #[test]
    fn gen_moves_can_revisit_end() {
        let graph = make_graph();
        // End node (2,2) already has degree 1 (visited via another path).
        // Head is at (1,2)=node 7, adjacent to end. The edge to end
        // (h_edge(1,2)=10) is NOT used — it should still be generated
        // because end nodes can always be revisited.
        let mut state = WitnessState::new(&graph);
        state.head = graph.node_xy_to_idx(1, 2); // node 7
        state.degrees[graph.end] = 1; // end was "visited" before

        let edge_to_end = graph.h_edge_index(1, 2);
        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);

        assert!(
            moves.contains(&edge_to_end),
            "move to end (edge {}) should be generated despite end degree > 0",
            edge_to_end
        );
    }

    #[test]
    fn gen_moves_cannot_revisit_non_end() {
        let graph = make_graph();
        let mut state = WitnessState::new(&graph);
        let mut undo = UndoStack::new();

        // Build a path that leaves a visited non-end node behind:
        // (0,0)→(1,0)→(2,0)→(2,1)→(1,1)
        state.apply_move(&graph, graph.h_edge_index(0, 0), &mut undo);
        state.apply_move(&graph, graph.h_edge_index(1, 0), &mut undo);
        state.apply_move(&graph, graph.v_edge_index(2, 0), &mut undo);
        state.apply_move(&graph, graph.h_edge_index(1, 1), &mut undo);

        // Head is now at (1,1)=node 4. Node 1=(1,0) has degree 2 and is NOT an end node.
        // v_edge(1,0)=3 connecting node 4→node 1 is unused — it should be excluded.
        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);

        let edge_back = graph.v_edge_index(1, 0); // = 3
        assert!(
            !moves.contains(&edge_back),
            "edge back to non-end visited node (edge {}) should be excluded",
            edge_back
        );
    }

    #[test]
    fn apply_move_undo_roundtrip() {
        let graph = make_graph();
        let mut state = WitnessState::new(&graph);
        let original = state.clone();
        let mut undo = UndoStack::new();

        state.apply_move(&graph, graph.h_edge_index(0, 0), &mut undo);

        undo.rollback(&mut state);

        assert_eq!(state.used_edges, original.used_edges);
        assert_eq!(state.degrees, original.degrees);
        assert_eq!(state.head, original.head);
    }

    #[test]
    fn apply_undo_cycles() {
        let graph = make_graph();
        let mut state = WitnessState::new(&graph);
        let original = state.clone();

        // ---- First cycle: 4 moves ----
        let mut undo = UndoStack::new();
        state.apply_move(&graph, graph.h_edge_index(0, 0), &mut undo);
        state.apply_move(&graph, graph.h_edge_index(1, 0), &mut undo);
        state.apply_move(&graph, graph.v_edge_index(2, 0), &mut undo);
        state.apply_move(&graph, graph.h_edge_index(1, 1), &mut undo);
        // Undo each move in reverse order (rollback pops one mark at a time)
        undo.rollback(&mut state);
        undo.rollback(&mut state);
        undo.rollback(&mut state);
        undo.rollback(&mut state);
        assert_eq!(
            state.used_edges, original.used_edges,
            "first cycle: used_edges corrupted"
        );
        assert_eq!(
            state.degrees, original.degrees,
            "first cycle: degrees corrupted"
        );
        assert_eq!(state.head, original.head, "first cycle: head corrupted");

        // ---- Second cycle: different path ----
        let mut undo = UndoStack::new();
        state.apply_move(&graph, graph.v_edge_index(0, 0), &mut undo);
        state.apply_move(&graph, graph.v_edge_index(0, 1), &mut undo);
        undo.rollback(&mut state);
        undo.rollback(&mut state);
        assert_eq!(
            state.used_edges, original.used_edges,
            "second cycle: used_edges corrupted"
        );
        assert_eq!(
            state.degrees, original.degrees,
            "second cycle: degrees corrupted"
        );
        assert_eq!(state.head, original.head, "second cycle: head corrupted");
    }

    #[test]
    fn is_satisfied_head_at_end() {
        let graph = make_graph();
        let validator = WitnessValidator::new(&graph);
        let mut state = WitnessState::new(&graph);
        let mut undo = UndoStack::new();

        // Build a complete valid path from (0,0) to (2,2):
        // (0,0)→(0,1)→(0,2)→(1,2)→(2,2)
        state.apply_move(&graph, graph.v_edge_index(0, 0), &mut undo);
        state.apply_move(&graph, graph.v_edge_index(0, 1), &mut undo);
        state.apply_move(&graph, graph.h_edge_index(0, 2), &mut undo);
        state.apply_move(&graph, graph.h_edge_index(1, 2), &mut undo);

        assert_eq!(state.head, graph.end);
        assert!(validator.is_satisfied(&state, &graph));
    }

    #[test]
    fn is_satisfied_head_not_at_end() {
        let graph = make_graph();
        let validator = WitnessValidator::new(&graph);
        let mut state = WitnessState::new(&graph);
        let mut undo = UndoStack::new();

        // Partial path: (0,0)→(0,1)→(0,2) — head has NOT reached end yet
        state.apply_move(&graph, graph.v_edge_index(0, 0), &mut undo);
        state.apply_move(&graph, graph.v_edge_index(0, 1), &mut undo);

        assert_ne!(state.head, graph.end);
        assert!(!validator.is_satisfied(&state, &graph));
    }
}
