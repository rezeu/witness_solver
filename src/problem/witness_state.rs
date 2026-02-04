use crate::problem::graph::Graph;
use crate::state::SearchState;

#[derive(Clone)]
pub struct WitnessState {
    pub used_edges: Vec<u64>,
    pub degrees: Vec<u8>,
    pub head: usize,
}
pub enum UndoEntry {
    DecDeg { node_index: usize },
    ClearEdgeBit { edge_index: usize },
    Head { node_index: usize },
}
impl std::fmt::Display for UndoEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UndoEntry::DecDeg { node_index } => {
                write!(f, "DecDeg {{ node_index: {} }}", node_index)
            }
            UndoEntry::ClearEdgeBit { edge_index } => {
                write!(f, "ClearEdgeBit {{ edge_index: {} }}", edge_index)
            }
            UndoEntry::Head { node_index } => {
                write!(f, "Head {{ node_index: {} }}", node_index)
            }
        }
    }
}
impl WitnessState {
    pub fn new(g: &Graph) -> Self {
        let num_edges = g.width * (g.height + 1) + g.height * (g.width + 1);
        let used_edges = vec![0u64; div_ceil(num_edges, 64)];
        let degrees = vec![0u8; (g.width + 1) * (g.height + 1)];
        WitnessState {
            used_edges,
            degrees,
            head: g.start,
        }
    }
    fn node_idx_to_xy(&self, g: &Graph, ni: usize) -> (usize, usize) {
        g.node_idx_to_xy(ni)
    }
    fn node_xy_to_idx(&self, g: &Graph, x: usize, y: usize) -> usize {
        g.node_xy_to_idx(x, y)
    }
    fn edge_idx_to_endpoints(&self, g: &Graph, ei: usize) -> (usize, usize) {
        g.edge_idx_to_endpoints(ei)
    }
    fn edge_endpoints_to_idx(&self, g: &Graph, u: usize, v: usize) -> usize {
        g.edge_endpoints_to_idx(u, v)
    }
    fn used(&self, ei: usize) -> bool {
        test_bit(&self.used_edges, ei)
    }
    fn adj_nodes(&self, g: &Graph, u: usize) -> Vec<usize> {
        g.adj_nodes(u)
    }
}

impl SearchState for WitnessState {
    type Move = usize;
    type UndoEntry = UndoEntry;

    fn gen_moves(&self,g: &Graph, out: &mut Vec<Self::Move>) {
        let u = self.head;

        for v in self.adj_nodes(g, u) {
            let ei = self.edge_endpoints_to_idx(g, u, v);
            if !self.used(ei) {
                out.push(ei);
            }
        }
    }

    fn apply_move(&mut self, g: &Graph, mv: Self::Move, undost: &mut crate::undo::UndoStack<Self>) {
        undost.mark();

        let (u, v) = self.edge_idx_to_endpoints(g, mv);
        debug_assert!(!test_bit(&self.used_edges, mv));

        undost.push(UndoEntry::ClearEdgeBit { edge_index: mv });
        set_bit(&mut self.used_edges, mv);

        undost.push(UndoEntry::DecDeg { node_index: u });
        undost.push(UndoEntry::DecDeg { node_index: v });
        self.degrees[u] += 1;
        self.degrees[v] += 1;

        debug_assert!(self.head == u || self.head == v, "Head: {}, u: {}, v: {}, mv: {}", self.head, u, v, mv);
        undost.push(UndoEntry::Head {
            node_index: self.head,
        });
        self.head = if self.head == u { v } else { u };
    }

    fn apply_undo(&mut self, entry: Self::UndoEntry) {
        match entry {
            UndoEntry::ClearEdgeBit { edge_index } => {
                clear_bit(&mut self.used_edges, edge_index);
            }
            UndoEntry::DecDeg { node_index } => {
                self.degrees[node_index] -= 1;
            }
            UndoEntry::Head { node_index } => {
                self.head = node_index;
            }
        }
    }

    fn draw(&self, g: &Graph) {
        g.draw_with_state(Some(self));
    }
}

#[inline(always)]
fn div_ceil(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

#[inline(always)]
pub fn test_bit(bits: &Vec<u64>, i: usize) -> bool {
    let w = i >> 6;
    let b = i & 63;
    ((bits[w] >> b) & 1) != 0
}

#[inline(always)]
fn set_bit(bits: &mut Vec<u64>, i: usize) {
    let w = i >> 6;
    let b = i & 63;
    bits[w] |= 1u64 << b;
}

#[inline(always)]
fn clear_bit(bits: &mut Vec<u64>, i: usize) {
    let w = i >> 6;
    let b = i & 63;
    bits[w] &= !(1u64 << b);
}
