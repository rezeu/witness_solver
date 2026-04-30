use serde::Deserialize;
use std::fs;

// ---------------------------------------------------------------------------
// JSON schema
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PuzzleJson {
    pub width: usize,
    pub height: usize,
    pub starts: Vec<[usize; 2]>,
    pub ends: Vec<[usize; 2]>,
    #[serde(default)]
    pub node_dots: Vec<[usize; 2]>,
    #[serde(default)]
    pub edge_dots: Vec<[[usize; 2]; 2]>,
    #[serde(default)]
    pub broken_edges: Vec<[[usize; 2]; 2]>,
    #[serde(default)]
    pub squares: Vec<SquareJson>,
    #[serde(default)]
    pub stars: Vec<StarJson>,
    #[serde(default)]
    pub triangles: Vec<TriangleJson>,
    #[serde(default)]
    pub tetris: Vec<TetrisJson>,
    #[serde(default)]
    pub eliminations: Vec<[usize; 2]>,
    #[serde(default)]
    pub symmetry: Option<SymmetryKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SymmetryKind {
    #[serde(rename = "x")]
    MirrorX,
    #[serde(rename = "y")]
    MirrorY,
    #[serde(rename = "xy")]
    MirrorXY,
}

#[derive(Deserialize)]
pub struct SquareJson {
    pub pos: [usize; 2],
    pub color: u8,
}

#[derive(Deserialize)]
pub struct StarJson {
    pub pos: [usize; 2],
    pub color: u8,
}

#[derive(Deserialize)]
pub struct TriangleJson {
    pub pos: [usize; 2],
    pub count: u8,
}

#[derive(Deserialize)]
pub struct TetrisJson {
    pub pos: [usize; 2],
    pub shape: Vec<[i8; 2]>,
    #[serde(default)]
    pub negative: bool,
}

// ---------------------------------------------------------------------------
// Cell constraint enum
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum CellConstraint {
    None,
    Square { color: u8 },
    Star { color: u8 },
    Triangle { count: u8 },
    Tetris { shape: Vec<[i8; 2]>, negative: bool },
    Elimination,
}

impl Default for CellConstraint {
    fn default() -> Self {
        CellConstraint::None
    }
}

// ---------------------------------------------------------------------------
// Processed graph
// ---------------------------------------------------------------------------

pub struct WitnessGraph {
    pub width: usize,
    pub height: usize,
    pub start: usize,              // node index
    pub end: usize,                // node index
    pub broken: Vec<u64>,          // bitset of broken edge indices
    pub dot_nodes: Vec<usize>,     // list of node indices with dots
    pub dot_edges: Vec<usize>,     // list of edge indices with dots
    pub cells: Vec<CellConstraint>,// cy * width + cx
    pub has_region_rules: bool,    // any cell has square/star/tetris/elimination
    pub triangle_cells: Vec<(usize, usize, u8)>, // (cx, cy, count) for early pruning
    /// Pre-computed adjacency: adj[node] = ([neighbor; 4], count).
    /// Avoids division/modulo in hot paths (gen_moves, pruner BFS).
    pub adj: Vec<([usize; 4], u8)>,
    pub symmetry: Option<SymmetryKind>,
}

// --- Bitset helpers (small, for broken-edge set) --------------------------

#[inline(always)]
fn test_bit(bits: &[u64], i: usize) -> bool {
    let w = i >> 6;
    let b = i & 63;
    w < bits.len() && ((bits[w] >> b) & 1) != 0
}

#[inline(always)]
fn set_bit(bits: &mut [u64], i: usize) {
    let w = i >> 6;
    let b = i & 63;
    if w < bits.len() {
        bits[w] |= 1u64 << b;
    }
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl WitnessGraph {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let text = fs::read_to_string(path)?;
        let json: PuzzleJson = serde_json::from_str(&text)?;
        Self::from_json(json)
    }

    pub fn from_json(j: PuzzleJson) -> Result<Self, Box<dyn std::error::Error>> {
        let w = j.width;
        let h = j.height;

        if j.starts.is_empty() {
            return Err("need at least one start".into());
        }
        if j.ends.is_empty() {
            return Err("need at least one end".into());
        }

        let start_xy = j.starts[0];
        let end_xy = j.ends[0];

        let start = Self::xy_to_idx_static(w, start_xy[0], start_xy[1]);
        let end = Self::xy_to_idx_static(w, end_xy[0], end_xy[1]);

        let num_slots = Self::num_edge_slots_static(w, h);
        let bitset_words = (num_slots + 63) / 64;

        // Broken edges
        let mut broken = vec![0u64; bitset_words];
        for pair in &j.broken_edges {
            let u = Self::xy_to_idx_static(w, pair[0][0], pair[0][1]);
            let v = Self::xy_to_idx_static(w, pair[1][0], pair[1][1]);
            let ei = Self::edge_endpoints_to_idx_static(w, u, v);
            set_bit(&mut broken, ei);
        }

        // Dot nodes
        let dot_nodes: Vec<usize> = j
            .node_dots
            .iter()
            .map(|xy| Self::xy_to_idx_static(w, xy[0], xy[1]))
            .collect();

        // Dot edges
        let dot_edges: Vec<usize> = j
            .edge_dots
            .iter()
            .map(|pair| {
                let u = Self::xy_to_idx_static(w, pair[0][0], pair[0][1]);
                let v = Self::xy_to_idx_static(w, pair[1][0], pair[1][1]);
                Self::edge_endpoints_to_idx_static(w, u, v)
            })
            .collect();

        // Cells
        let mut cells = vec![CellConstraint::None; w * h];
        for sq in &j.squares {
            let idx = sq.pos[1] * w + sq.pos[0];
            cells[idx] = CellConstraint::Square { color: sq.color };
        }
        for st in &j.stars {
            let idx = st.pos[1] * w + st.pos[0];
            cells[idx] = CellConstraint::Star { color: st.color };
        }
        for tr in &j.triangles {
            let idx = tr.pos[1] * w + tr.pos[0];
            cells[idx] = CellConstraint::Triangle { count: tr.count };
        }
        for te in &j.tetris {
            let idx = te.pos[1] * w + te.pos[0];
            cells[idx] = CellConstraint::Tetris {
                shape: te.shape.clone(),
                negative: te.negative,
            };
        }
        for el in &j.eliminations {
            let idx = el[1] * w + el[0];
            cells[idx] = CellConstraint::Elimination;
        }

        let has_region_rules = cells.iter().any(|c| matches!(c,
            CellConstraint::Square { .. } | CellConstraint::Star { .. } |
            CellConstraint::Tetris { .. } | CellConstraint::Elimination
        ));

        // Pre-compute triangle cells for early pruning
        let mut triangle_cells = Vec::new();
        for cy in 0..h {
            for cx in 0..w {
                if let CellConstraint::Triangle { count } = cells[cy * w + cx] {
                    triangle_cells.push((cx, cy, count));
                }
            }
        }

        // Pre-compute adjacency list (avoids division/modulo in hot paths)
        let num_nodes = (w + 1) * (h + 1);
        let mut adj = vec![([0usize; 4], 0u8); num_nodes];
        for y in 0..=h {
            for x in 0..=w {
                let ni = y * (w + 1) + x;
                let mut neighbors = [0usize; 4];
                let mut count = 0u8;
                if x > 0 {
                    neighbors[count as usize] = ni - 1;
                    count += 1;
                }
                if x < w {
                    neighbors[count as usize] = ni + 1;
                    count += 1;
                }
                if y > 0 {
                    neighbors[count as usize] = ni - (w + 1);
                    count += 1;
                }
                if y < h {
                    neighbors[count as usize] = ni + (w + 1);
                    count += 1;
                }
                adj[ni] = (neighbors, count);
            }
        }

        Ok(WitnessGraph {
            width: w,
            height: h,
            start,
            end,
            broken,
            dot_nodes,
            dot_edges,
            cells,
            has_region_rules,
            triangle_cells,
            adj,
            symmetry: j.symmetry,
        })
    }

    // --- Static helpers (no &self, used during construction) ---------------

    fn xy_to_idx_static(w: usize, x: usize, y: usize) -> usize {
        y * (w + 1) + x
    }

    fn num_edge_slots_static(w: usize, h: usize) -> usize {
        2 * (h + 1) * (w + 1)
    }

    fn edge_endpoints_to_idx_static(w: usize, u: usize, v: usize) -> usize {
        let ux = u % (w + 1);
        let uy = u / (w + 1);
        let vx = v % (w + 1);
        let vy = v / (w + 1);
        if uy == vy {
            // horizontal
            let x = usize::min(ux, vx);
            let y = uy;
            2 * (y * w + x)
        } else {
            // vertical
            let x = ux;
            let y = usize::min(uy, vy);
            2 * (y * (w + 1) + x) + 1
        }
    }

    // --- Instance methods -------------------------------------------------

    #[inline]
    pub fn num_nodes(&self) -> usize {
        (self.width + 1) * (self.height + 1)
    }

    #[inline]
    pub fn num_edge_slots(&self) -> usize {
        Self::num_edge_slots_static(self.width, self.height)
    }

    #[inline]
    pub fn node_xy_to_idx(&self, x: usize, y: usize) -> usize {
        y * (self.width + 1) + x
    }

    #[inline]
    pub fn node_idx_to_xy(&self, ni: usize) -> (usize, usize) {
        (ni % (self.width + 1), ni / (self.width + 1))
    }

    #[inline]
    pub fn h_edge_index(&self, x: usize, y: usize) -> usize {
        2 * (y * self.width + x)
    }

    #[inline]
    pub fn v_edge_index(&self, x: usize, y: usize) -> usize {
        2 * (y * (self.width + 1) + x) + 1
    }

    pub fn edge_idx_to_endpoints(&self, ei: usize) -> (usize, usize) {
        let real = ei >> 1;
        if ei & 1 == 0 {
            // horizontal
            let y = real / self.width;
            let x = real % self.width;
            let u = self.node_xy_to_idx(x, y);
            let v = self.node_xy_to_idx(x + 1, y);
            (u, v)
        } else {
            // vertical
            let y = real / (self.width + 1);
            let x = real % (self.width + 1);
            let u = self.node_xy_to_idx(x, y);
            let v = self.node_xy_to_idx(x, y + 1);
            (u, v)
        }
    }

    #[inline]
    pub fn edge_endpoints_to_idx(&self, u: usize, v: usize) -> usize {
        Self::edge_endpoints_to_idx_static(self.width, u, v)
    }

    #[inline]
    pub fn is_broken(&self, ei: usize) -> bool {
        test_bit(&self.broken, ei)
    }

    #[inline]
    pub fn cell(&self, cx: usize, cy: usize) -> &CellConstraint {
        &self.cells[cy * self.width + cx]
    }

    /// Iterate all grid-adjacent nodes of `u` via closure (zero-allocation).
    /// Uses pre-computed adjacency list — no division/modulo.
    #[inline]
    pub fn for_each_neighbor(&self, u: usize, mut f: impl FnMut(usize)) {
        let (neighbors, count) = &self.adj[u];
        for i in 0..*count as usize {
            f(neighbors[i]);
        }
    }
    /// If the puzzle has symmetry, return the mirror node for `ni`.
    /// Returns `None` when the node is on the symmetry axis (self-symmetric).
    pub fn symmetric_node(&self, ni: usize) -> Option<usize> {
        let kind = self.symmetry?;
        let (x, y) = self.node_idx_to_xy(ni);
        match kind {
            SymmetryKind::MirrorX => {
                if 2 * x == self.width {
                    None
                } else {
                    Some(self.node_xy_to_idx(self.width - x, y))
                }
            }
            SymmetryKind::MirrorY => {
                if 2 * y == self.height {
                    None
                } else {
                    Some(self.node_xy_to_idx(x, self.height - y))
                }
            }
            SymmetryKind::MirrorXY => {
                if 2 * x == self.width && 2 * y == self.height {
                    None
                } else {
                    Some(self.node_xy_to_idx(self.width - x, self.height - y))
                }
            }
        }
    }

    /// If the puzzle has symmetry, return the mirror edge for `ei`.
    /// Returns `None` when the edge is self-symmetric (lies on the symmetry axis).
    pub fn symmetric_edge(&self, ei: usize) -> Option<usize> {
        let (u, v) = self.edge_idx_to_endpoints(ei);
        let mu = self.symmetric_node(u);
        let mv = self.symmetric_node(v);

        // Both endpoints on-axis → edge is fully on-axis → self-symmetric
        if mu.is_none() && mv.is_none() {
            return None;
        }

        let mu = mu.unwrap_or(u);
        let mv = mv.unwrap_or(v);

        // Mirrored endpoints same as original → self-symmetric
        if mu == u && mv == v {
            return None;
        }

        Some(self.edge_endpoints_to_idx(mu, mv))
    }

    /// Return all path endpoint node indices for this puzzle.
    /// For non-symmetry: [start, end]. For symmetry: [start, end, mirror_start, mirror_end],
    /// with duplicates when a node serves as both player and mirror endpoint
    /// (self-symmetric on-axis nodes), so that the degree check can expect
    /// degree 1 for unique endpoints and degree 2 for double-role endpoints.
    pub fn all_end_nodes(&self) -> Vec<usize> {
        let mut nodes = vec![self.start, self.end];
        if self.symmetry.is_some() {
            // Mirror start
            match self.symmetric_node(self.start) {
                Some(ms) => {
                    if !nodes.contains(&ms) {
                        nodes.push(ms);
                    }
                }
                None => {
                    // self.start is on the symmetry axis: it serves as both
                    // player start and mirror start → expect degree 2
                    nodes.push(self.start);
                }
            }
            // Mirror end
            match self.symmetric_node(self.end) {
                Some(me) => {
                    if !nodes.contains(&me) {
                        nodes.push(me);
                    }
                }
                None => {
                    // self.end is on the symmetry axis: it serves as both
                    // player end and mirror end → expect degree 2
                    nodes.push(self.end);
                }
            }
        }
        nodes
    }
}

// Safety: WitnessGraph is immutable after construction and contains no
// interior mutability, so it's safe to share across threads.
unsafe impl Sync for WitnessGraph {}
unsafe impl Send for WitnessGraph {}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helpers -----------------------------------------------------------

    /// Build a non-symmetry `w`×`h` puzzle (start at (0,0), end at (w,h)).
    fn make_graph(w: usize, h: usize) -> WitnessGraph {
        WitnessGraph::from_json(PuzzleJson {
            width: w,
            height: h,
            starts: vec![[0, 0]],
            ends: vec![[w, h]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        })
        .unwrap()
    }

    /// Build a symmetry puzzle with given kind.
    fn make_symmetry_graph(sym: SymmetryKind, w: usize, h: usize) -> WitnessGraph {
        WitnessGraph::from_json(PuzzleJson {
            width: w,
            height: h,
            starts: vec![[0, 0]],
            ends: vec![[w, h]],
            symmetry: Some(sym),
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        })
        .unwrap()
    }

    // =======================================================================
    // Edge Indexing Tests
    // =======================================================================

    #[test]
    fn test_h_edge_index_formula() {
        let g = make_graph(4, 4);
        // h_edge(x,y) = 2*(y*w + x) = 2*(4y + x)
        assert_eq!(g.h_edge_index(0, 0), 0);
        assert_eq!(g.h_edge_index(1, 0), 2);
        assert_eq!(g.h_edge_index(2, 0), 4);
        assert_eq!(g.h_edge_index(3, 0), 6);
        assert_eq!(g.h_edge_index(0, 1), 8);
        assert_eq!(g.h_edge_index(1, 2), 2 * (2 * 4 + 1));
        assert_eq!(g.h_edge_index(3, 4), 2 * (4 * 4 + 3));

        let g3 = make_graph(3, 3);
        assert_eq!(g3.h_edge_index(0, 0), 0);
        assert_eq!(g3.h_edge_index(1, 0), 2);
        assert_eq!(g3.h_edge_index(2, 3), 22);
    }

    #[test]
    fn test_v_edge_index_formula() {
        let g = make_graph(4, 4);
        // v_edge(x,y) = 2*(y*(w+1) + x) + 1 = 2*(5y + x) + 1
        assert_eq!(g.v_edge_index(0, 0), 1);
        assert_eq!(g.v_edge_index(1, 0), 3);
        assert_eq!(g.v_edge_index(4, 0), 9);
        assert_eq!(g.v_edge_index(0, 1), 11);
        assert_eq!(g.v_edge_index(4, 3), 39);

        let g3 = make_graph(3, 3);
        assert_eq!(g3.v_edge_index(0, 0), 1);
        assert_eq!(g3.v_edge_index(3, 0), 7);
        assert_eq!(g3.v_edge_index(3, 2), 23);
    }

    #[test]
    fn test_edge_indices_unique() {
        // h_edge produces evens, v_edge produces odds — so they never collide.
        let g = make_graph(4, 4);
        for x in 0..4 {
            for y in 0..4 {
                let h = g.h_edge_index(x, y);
                let v = g.v_edge_index(x, y);
                assert_ne!(h, v, "h_edge({},{})={} collides with v_edge({},{})={}", x, y, h, x, y, v);
                assert!(h % 2 == 0, "h_edge should be even, got {}", h);
                assert!(v % 2 == 1, "v_edge should be odd, got {}", v);
            }
        }
    }

    #[test]
    fn test_edge_indices_in_bounds() {
        let g = make_graph(4, 4);
        let max_h = g.h_edge_index(3, 4); // x=w-1=3, y=h=4
        let max_v = g.v_edge_index(4, 3); // x=w=4, y=h-1=3
        assert!(max_h < g.num_edge_slots());
        assert!(max_v < g.num_edge_slots());
        assert_eq!(g.num_edge_slots(), 50);
        assert!(max_h == 38);
        assert!(max_v == 39);
    }

    #[test]
    fn test_edge_idx_to_endpoints_roundtrip() {
        let g = make_graph(4, 4);
        let (u, v) = g.edge_idx_to_endpoints(0);
        assert_eq!(u, g.node_xy_to_idx(0, 0));
        assert_eq!(v, g.node_xy_to_idx(1, 0));

        let (u, v) = g.edge_idx_to_endpoints(38);
        assert_eq!(u, g.node_xy_to_idx(3, 4));
        assert_eq!(v, g.node_xy_to_idx(4, 4));

        let (u, v) = g.edge_idx_to_endpoints(1);
        assert_eq!(u, g.node_xy_to_idx(0, 0));
        assert_eq!(v, g.node_xy_to_idx(0, 1));

        let (u, v) = g.edge_idx_to_endpoints(39);
        assert_eq!(u, g.node_xy_to_idx(4, 3));
        assert_eq!(v, g.node_xy_to_idx(4, 4));
    }

    #[test]
    fn test_edge_endpoints_to_idx_roundtrip() {
        let g = make_graph(4, 4);
        let u = g.node_xy_to_idx(0, 0);
        let v = g.node_xy_to_idx(1, 0);
        assert_eq!(g.edge_endpoints_to_idx(u, v), 0);
        assert_eq!(g.edge_endpoints_to_idx(v, u), 0, "direction-agnostic");

        let u = g.node_xy_to_idx(0, 0);
        let v = g.node_xy_to_idx(0, 1);
        assert_eq!(g.edge_endpoints_to_idx(u, v), 1);
        assert_eq!(g.edge_endpoints_to_idx(v, u), 1);

        let u = g.node_xy_to_idx(4, 3);
        let v = g.node_xy_to_idx(4, 4);
        assert_eq!(g.edge_endpoints_to_idx(u, v), 39);
    }

    #[test]
    fn test_num_edge_slots() {
        assert_eq!(make_graph(4, 4).num_edge_slots(), 2 * 5 * 5);
        assert_eq!(make_graph(3, 3).num_edge_slots(), 2 * 4 * 4);
        assert_eq!(make_graph(2, 2).num_edge_slots(), 2 * 3 * 3);
        assert_eq!(make_graph(1, 1).num_edge_slots(), 2 * 2 * 2);
        assert_eq!(make_graph(6, 6).num_edge_slots(), 2 * 7 * 7);
    }

    #[test]
    fn test_valid_edge_indices_exhaustive() {
        let g = make_graph(4, 4);
        let num_nodes = g.num_nodes();
        let num_slots = g.num_edge_slots();
        // For 4×4: indices 0-39 are valid edges, 40-49 are padding.
        for ei in 0..num_slots {
            let (u, v) = g.edge_idx_to_endpoints(ei);
            let roundtrip = g.edge_endpoints_to_idx(u, v);
            assert_eq!(
                roundtrip, ei,
                "edge {} roundtripped to {} via endpoints ({},{})",
                ei, roundtrip, u, v
            );
            // Valid edges have both endpoints in node range
            let endpoints_valid = u < num_nodes && v < num_nodes;
            let is_padding = ei >= 40;
            assert_eq!(
                endpoints_valid, !is_padding,
                "edge {}: u={} v={} valid={} expected_valid={}",
                ei, u, v, endpoints_valid, !is_padding
            );
        }
    }

    // =======================================================================
    // Node Indexing Tests
    // =======================================================================

    #[test]
    fn test_node_xy_to_idx() {
        let g = make_graph(4, 4);
        assert_eq!(g.node_xy_to_idx(0, 0), 0);
        assert_eq!(g.node_xy_to_idx(1, 0), 1);
        assert_eq!(g.node_xy_to_idx(4, 0), 4);
        assert_eq!(g.node_xy_to_idx(0, 1), 5);
        assert_eq!(g.node_xy_to_idx(4, 4), 24);
        assert_eq!(g.node_xy_to_idx(2, 3), 3 * 5 + 2);
    }

    #[test]
    fn test_node_idx_to_xy() {
        let g = make_graph(4, 4);
        assert_eq!(g.node_idx_to_xy(0), (0, 0));
        assert_eq!(g.node_idx_to_xy(4), (4, 0));
        assert_eq!(g.node_idx_to_xy(5), (0, 1));
        assert_eq!(g.node_idx_to_xy(24), (4, 4));
        assert_eq!(g.node_idx_to_xy(17), (2, 3));
    }

    #[test]
    fn test_node_roundtrip() {
        let g = make_graph(4, 4);
        for ni in 0..g.num_nodes() {
            let (x, y) = g.node_idx_to_xy(ni);
            assert_eq!(g.node_xy_to_idx(x, y), ni);
        }
    }

    #[test]
    fn test_num_nodes() {
        assert_eq!(make_graph(4, 4).num_nodes(), 5 * 5);
        assert_eq!(make_graph(3, 3).num_nodes(), 4 * 4);
        assert_eq!(make_graph(2, 2).num_nodes(), 3 * 3);
        assert_eq!(make_graph(1, 1).num_nodes(), 2 * 2);
        assert_eq!(make_graph(6, 6).num_nodes(), 7 * 7);
    }

    // =======================================================================
    // Symmetry Tests
    // =======================================================================

    #[test]
    fn test_symmetric_node_mirror_x() {
        let g = make_symmetry_graph(SymmetryKind::MirrorX, 4, 4);
        // Off-axis nodes mirror across x=2
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(0, 0)).unwrap(), g.node_xy_to_idx(4, 0));
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(4, 0)).unwrap(), g.node_xy_to_idx(0, 0));
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(1, 3)).unwrap(), g.node_xy_to_idx(3, 3));
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(3, 3)).unwrap(), g.node_xy_to_idx(1, 3));
        // On-axis nodes (x=2) return None
        assert!(g.symmetric_node(g.node_xy_to_idx(2, 0)).is_none());
        assert!(g.symmetric_node(g.node_xy_to_idx(2, 4)).is_none());
    }

    #[test]
    fn test_symmetric_node_mirror_y() {
        let g = make_symmetry_graph(SymmetryKind::MirrorY, 4, 4);
        // Off-axis nodes mirror across y=2
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(0, 0)).unwrap(), g.node_xy_to_idx(0, 4));
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(0, 4)).unwrap(), g.node_xy_to_idx(0, 0));
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(3, 1)).unwrap(), g.node_xy_to_idx(3, 3));
        // On-axis nodes (y=2) return None
        assert!(g.symmetric_node(g.node_xy_to_idx(0, 2)).is_none());
        assert!(g.symmetric_node(g.node_xy_to_idx(4, 2)).is_none());
    }

    #[test]
    fn test_symmetric_node_mirror_xy() {
        let g = make_symmetry_graph(SymmetryKind::MirrorXY, 4, 4);
        // Off-axis nodes mirror across (x=2, y=2)
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(0, 0)).unwrap(), g.node_xy_to_idx(4, 4));
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(4, 4)).unwrap(), g.node_xy_to_idx(0, 0));
        assert_eq!(g.symmetric_node(g.node_xy_to_idx(3, 1)).unwrap(), g.node_xy_to_idx(1, 3));
        // On-axis node (x=2, y=2) returns None
        assert!(g.symmetric_node(g.node_xy_to_idx(2, 2)).is_none());
        // Nodes on one axis but not both: off-axis
        assert!(g.symmetric_node(g.node_xy_to_idx(2, 0)).is_some());
        assert!(g.symmetric_node(g.node_xy_to_idx(0, 2)).is_some());
    }

    #[test]
    fn test_symmetric_node_odd_grid() {
        // 3×3 grid (w=3, odd): MirrorX axis runs between nodes, no integer x satisfies 2*x==3.
        let g = make_symmetry_graph(SymmetryKind::MirrorX, 3, 3);
        for node_idx in 0..g.num_nodes() {
            let mirror = g
                .symmetric_node(node_idx)
                .expect("all nodes should have a mirror in odd-width MirrorX");
            assert_ne!(
                mirror, node_idx,
                "node {} should NOT be self-symmetric (no on-axis nodes for odd width)",
                node_idx
            );
        }
    }

    #[test]
    fn test_symmetric_node_involution() {
        let g = make_symmetry_graph(SymmetryKind::MirrorXY, 4, 4);
        for node_idx in 0..g.num_nodes() {
            if let Some(m) = g.symmetric_node(node_idx) {
                assert_eq!(
                    g.symmetric_node(m).unwrap(),
                    node_idx,
                    "mirror of mirror of node {} should be itself",
                    node_idx
                );
            }
        }
    }

    #[test]
    fn test_symmetric_node_no_symmetry() {
        let g = make_graph(4, 4);
        for node_idx in 0..g.num_nodes() {
            assert!(
                g.symmetric_node(node_idx).is_none(),
                "no node should have a mirror when symmetry is None"
            );
        }
    }

    #[test]
    fn test_symmetric_edge_mirror_x() {
        let g = make_symmetry_graph(SymmetryKind::MirrorX, 4, 4);
        let original = g.h_edge_index(0, 1);
        let expected = g.h_edge_index(3, 1);
        assert_eq!(g.symmetric_edge(original).unwrap(), expected);
        assert_eq!(g.symmetric_edge(expected).unwrap(), original);

        let original = g.v_edge_index(1, 1);
        let expected = g.v_edge_index(3, 1);
        assert_eq!(g.symmetric_edge(original).unwrap(), expected);
    }

    #[test]
    fn test_symmetric_edge_self_symmetric() {
        // MirrorX 4×4: edge on the axis (v_edge at x=2) is self-symmetric.
        let g = make_symmetry_graph(SymmetryKind::MirrorX, 4, 4);
        for y in 0..4 {
            let axis_edge = g.v_edge_index(2, y);
            assert!(
                g.symmetric_edge(axis_edge).is_none(),
                "v_edge(2,{}) should be self-symmetric",
                y
            );
        }
    }

    #[test]
    fn test_symmetric_edge_no_symmetry() {
        let g = make_graph(4, 4);
        // All edges should be self-symmetric (no mirror) when symmetry is None.
        for y in 0..4 {
            for x in 0..4 {
                assert!(g.symmetric_edge(g.h_edge_index(x, y)).is_none());
                assert!(g.symmetric_edge(g.v_edge_index(x, y)).is_none());
            }
        }
    }

    #[test]
    fn test_symmetric_edge_double_mirror() {
        // Mirror of mirror edge should be the original edge (for off-axis edges).
        let g = make_symmetry_graph(SymmetryKind::MirrorXY, 4, 4);
        for y in 0..4 {
            for x in 0..4 {
                let he = g.h_edge_index(x, y);
                if let Some(m) = g.symmetric_edge(he) {
                    assert_eq!(g.symmetric_edge(m).unwrap(), he,
                        "double mirror of h_edge({},{}) should be itself", x, y);
                }
                let ve = g.v_edge_index(x, y);
                if let Some(m) = g.symmetric_edge(ve) {
                    assert_eq!(g.symmetric_edge(m).unwrap(), ve,
                        "double mirror of v_edge({},{}) should be itself", x, y);
                }
            }
        }
    }

    // =======================================================================
    // End Node Tests
    // =======================================================================

    #[test]
    fn test_all_end_nodes_no_symmetry() {
        let g = make_graph(4, 4);
        let ends = g.all_end_nodes();
        assert_eq!(ends.len(), 2);
        assert_eq!(ends[0], g.start);
        assert_eq!(ends[1], g.end);
    }

    #[test]
    fn test_all_end_nodes_mirror_x() {
        // Symmetry: MirrorX, start=(0,0) off-axis, end=(4,4) off-axis
        // Expect [start, end, mirror_start, mirror_end] (4 unique)
        let g = make_symmetry_graph(SymmetryKind::MirrorX, 4, 4);
        let ends = g.all_end_nodes();
        assert_eq!(ends.len(), 4);
        // Contains originals
        assert!(ends.contains(&g.start));
        assert!(ends.contains(&g.end));
        // Contains mirrors
        assert!(ends.contains(&g.symmetric_node(g.start).unwrap()));
        assert!(ends.contains(&g.symmetric_node(g.end).unwrap()));
    }

    #[test]
    fn test_all_end_nodes_start_on_axis() {
        // MirrorX 4×4, start at (2,0) (on-axis), end at (4,2) (off-axis)
        // Expect [start, end, start_dup, mirror_end] = 4 entries, start appears twice
        let g = WitnessGraph::from_json(PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[2, 0]],
            ends: vec![[4, 2]],
            symmetry: Some(SymmetryKind::MirrorX),
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        })
        .unwrap();

        let ends = g.all_end_nodes();
        assert_eq!(ends.len(), 4);
        // start should appear twice (once as origin, once as mirror-of-self)
        let start = g.node_xy_to_idx(2, 0);
        let count_start = ends.iter().filter(|&&n| n == start).count();
        assert_eq!(count_start, 2, "on-axis start should be duplicated");
        // end should appear once, its mirror once
        let end = g.node_xy_to_idx(4, 2);
        let count_end = ends.iter().filter(|&&n| n == end).count();
        assert_eq!(count_end, 1);
        let mirror_end = g.symmetric_node(end).unwrap();
        assert!(ends.contains(&mirror_end));
    }

    #[test]
    fn test_all_end_nodes_end_on_axis() {
        // MirrorX 4×4, start=(0,0) (off-axis), end=(2,4) (on-axis)
        // Expect [start, end, mirror_start, end_dup] = 4 entries, end appears twice
        let g = WitnessGraph::from_json(PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[2, 4]],
            symmetry: Some(SymmetryKind::MirrorX),
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        })
        .unwrap();

        let ends = g.all_end_nodes();
        assert_eq!(ends.len(), 4);
        // end should appear twice
        let end = g.node_xy_to_idx(2, 4);
        let count_end = ends.iter().filter(|&&n| n == end).count();
        assert_eq!(count_end, 2, "on-axis end should be duplicated");
        // start appears once, its mirror once
        let start = g.node_xy_to_idx(0, 0);
        let count_start = ends.iter().filter(|&&n| n == start).count();
        assert_eq!(count_start, 1);
        let mirror_start = g.symmetric_node(start).unwrap();
        assert!(ends.contains(&mirror_start));
    }

    // =======================================================================
    // Misc Tests
    // =======================================================================

    #[test]
    fn test_is_broken_default() {
        let g = make_graph(4, 4);
        // No broken_edges in the puzzle → nothing is broken
        for ei in 0..40 {
            assert!(!g.is_broken(ei), "edge {} should not be broken", ei);
        }
    }

    #[test]
    fn test_is_broken_with_edge() {
        let g = WitnessGraph::from_json(PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[4, 4]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![[[0, 0], [1, 0]], [[2, 1], [2, 2]]],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        })
        .unwrap();

        assert!(g.is_broken(g.h_edge_index(0, 0)));
        assert!(g.is_broken(g.v_edge_index(2, 1)));
        assert!(!g.is_broken(g.h_edge_index(1, 0)));
        assert!(!g.is_broken(g.v_edge_index(0, 0)));
    }

    #[test]
    fn test_cell_default() {
        let g = make_graph(4, 4);
        // All cells should be CellConstraint::None (the default)
        for y in 0..4 {
            for x in 0..4 {
                let cell = g.cell(x, y);
                assert!(matches!(cell, CellConstraint::None),
                    "cell({},{}) should be None, got {:?}", x, y, cell);
            }
        }
    }

    #[test]
    fn test_send_sync() {
        fn _assert_send<T: Send>() {}
        fn _assert_sync<T: Sync>() {}
        // Compile-time checks — if these don't compile, WitnessGraph broke thread-safety
        _assert_send::<WitnessGraph>();
        _assert_sync::<WitnessGraph>();
    }

    // =======================================================================
    // PuzzleJson Tests
    // =======================================================================

    #[test]
    fn test_puzzle_json_default() {
        // Minimal valid PuzzleJson parses successfully
        let g = WitnessGraph::from_json(PuzzleJson {
            width: 1,
            height: 1,
            starts: vec![[0, 0]],
            ends: vec![[1, 1]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        })
        .unwrap();
        assert_eq!(g.width, 1);
        assert_eq!(g.height, 1);
        assert!(g.symmetry.is_none());
    }

    #[test]
    fn test_puzzle_json_symmetry() {
        let jx: PuzzleJson = serde_json::from_str(
            r#"{"width":2,"height":2,"starts":[[0,0]],"ends":[[2,2]],"symmetry":"x"}"#,
        )
        .unwrap();
        assert_eq!(jx.symmetry, Some(SymmetryKind::MirrorX));

        let jy: PuzzleJson = serde_json::from_str(
            r#"{"width":2,"height":2,"starts":[[0,0]],"ends":[[2,2]],"symmetry":"y"}"#,
        )
        .unwrap();
        assert_eq!(jy.symmetry, Some(SymmetryKind::MirrorY));

        let jxy: PuzzleJson = serde_json::from_str(
            r#"{"width":2,"height":2,"starts":[[0,0]],"ends":[[2,2]],"symmetry":"xy"}"#,
        )
        .unwrap();
        assert_eq!(jxy.symmetry, Some(SymmetryKind::MirrorXY));

        let jnone: PuzzleJson = serde_json::from_str(
            r#"{"width":2,"height":2,"starts":[[0,0]],"ends":[[2,2]]}"#,
        )
        .unwrap();
        assert_eq!(jnone.symmetry, None);
    }

    #[test]
    fn test_from_json_invalid_grid() {
        // Empty starts → error
        let err = WitnessGraph::from_json(PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![],
            ends: vec![[4, 4]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        });
        assert!(err.is_err());

        // Empty ends → error
        let err = WitnessGraph::from_json(PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        });
        assert!(err.is_err());
    }

    // =======================================================================
    // Verification / Integration Tests
    // =======================================================================

    #[test]
    fn test_h_edge_endpoints_match_formula() {
        let g = make_graph(4, 4);
        for y in 0..=4 {
            for x in 0..4 {
                let ei = g.h_edge_index(x, y);
                let (u, v) = g.edge_idx_to_endpoints(ei);
                assert_eq!(u, g.node_xy_to_idx(x, y));
                assert_eq!(v, g.node_xy_to_idx(x + 1, y));
            }
        }
    }

    #[test]
    fn test_v_edge_endpoints_match_formula() {
        let g = make_graph(4, 4);
        for y in 0..4 {
            for x in 0..=4 {
                let ei = g.v_edge_index(x, y);
                let (u, v) = g.edge_idx_to_endpoints(ei);
                assert_eq!(u, g.node_xy_to_idx(x, y));
                assert_eq!(v, g.node_xy_to_idx(x, y + 1));
            }
        }
    }

    #[test]
    fn test_full_edge_roundtrip() {
        let g = make_graph(4, 4);
        // For every valid edge, edge_endpoints_to_idx on its endpoints = the edge index
        let last_h = g.h_edge_index(3, 4);
        let last_v = g.v_edge_index(4, 3);
        for ei in 0..=usize::max(last_h, last_v) {
            let (u, v) = g.edge_idx_to_endpoints(ei);
            // Only check edges with valid endpoints (skip padding)
            if u < g.num_nodes() && v < g.num_nodes() {
                assert_eq!(
                    g.edge_endpoints_to_idx(u, v), ei,
                    "roundtrip failed for edge {}", ei
                );
            }
        }
    }

    #[test]
    fn test_full_node_roundtrip() {
        let g = make_graph(4, 4);
        for x in 0..=4 {
            for y in 0..=4 {
                let ni = g.node_xy_to_idx(x, y);
                assert_eq!(g.node_idx_to_xy(ni), (x, y));
            }
        }
    }

    #[test]
    fn test_all_edges_match_formula() {
        // Verify that every h_edge and v_edge produced by the formulas
        // correctly roundtrips through edge_idx_to_endpoints and back.
        let g = make_graph(4, 4);
        for y in 0..=4 {
            for x in 0..4 {
                let ei = g.h_edge_index(x, y);
                let (u, v) = g.edge_idx_to_endpoints(ei);
                assert_eq!(
                    g.edge_endpoints_to_idx(u, v), ei,
                    "h_edge({},{}): roundtrip mismatch", x, y
                );
            }
        }
        for y in 0..4 {
            for x in 0..=4 {
                let ei = g.v_edge_index(x, y);
                let (u, v) = g.edge_idx_to_endpoints(ei);
                assert_eq!(
                    g.edge_endpoints_to_idx(u, v), ei,
                    "v_edge({},{}): roundtrip mismatch", x, y
                );
            }
        }
    }
}
