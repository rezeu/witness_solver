use crate::solver::{SearchState, UndoStack};
use crate::witness::graph::WitnessGraph;

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
    pub head: usize,
}

pub enum UndoEntry {
    ClearEdgeBit { edge_index: usize },
    DecDeg { node_index: usize },
    Head { prev: usize },
}

impl WitnessState {
    pub fn new(g: &WitnessGraph) -> Self {
        let num_slots = g.num_edge_slots();
        let used_edges = vec![0u64; (num_slots + 63) / 64];
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
    pub fn is_end_node(&self, graph: &WitnessGraph, ni: usize) -> bool {
        if ni == graph.end {
            return true;
        }
        if graph.symmetry.is_some() {
            if let Some(me) = graph.symmetric_node(graph.end) {
                if ni == me {
                    return true;
                }
            }
            if let Some(ms) = graph.symmetric_node(graph.start) {
                if ni == ms {
                    return true;
                }
            }
        }
        false
    }
}

impl SearchState for WitnessState {
    type Move = usize; // edge index
    type UndoEntry = UndoEntry;
    type Ctx = WitnessGraph;

    fn gen_moves(&self, ctx: &WitnessGraph, out: &mut Vec<Self::Move>) {
        if self.head == ctx.end {
            return;
        }
        let head = self.head;
        ctx.for_each_neighbor(head, |v| {
            let ei = ctx.edge_endpoints_to_idx(head, v);

            if self.used(ei) || ctx.is_broken(ei) {
                return;
            }
            if self.degrees[v] != 0 && !self.is_end_node(ctx, v) {
                return;
            }

            if ctx.symmetry.is_some() {
                if let Some(me) = ctx.symmetric_edge(ei) {
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
            }

            out.push(ei);
        });
    }

    fn apply_move(&mut self, ctx: &WitnessGraph, mv: Self::Move, undo: &mut UndoStack<Self>) {
        undo.mark();

        let (u, v) = ctx.edge_idx_to_endpoints(mv);

        debug_assert!(!test_bit(&self.used_edges, mv));
        undo.push(UndoEntry::ClearEdgeBit { edge_index: mv });
        set_bit(&mut self.used_edges, mv);

        undo.push(UndoEntry::DecDeg { node_index: u });
        self.degrees[u] += 1;
        undo.push(UndoEntry::DecDeg { node_index: v });
        self.degrees[v] += 1;

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
