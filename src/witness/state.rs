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
}

impl SearchState for WitnessState {
    type Move = usize; // edge index
    type UndoEntry = UndoEntry;
    type Ctx = WitnessGraph;

    fn gen_moves(&self, ctx: &WitnessGraph, out: &mut Vec<Self::Move>) {
        // If already at the end, no further moves.
        if self.head == ctx.end {
            return;
        }
        let head = self.head;
        ctx.for_each_neighbor(head, |v| {
            let ei = ctx.edge_endpoints_to_idx(head, v);
            // Edge must be unused, not broken, and target unvisited (or is the end).
            if !self.used(ei)
                && !ctx.is_broken(ei)
                && (self.degrees[v] == 0 || v == ctx.end)
            {
                out.push(ei);
            }
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
