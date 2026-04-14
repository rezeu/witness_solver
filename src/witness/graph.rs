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
    // symmetry: reserved for future
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

        let has_region_rules = cells.iter().any(|c| !matches!(c, CellConstraint::None));

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
    #[inline]
    pub fn for_each_neighbor(&self, u: usize, mut f: impl FnMut(usize)) {
        let (ux, uy) = self.node_idx_to_xy(u);
        if ux > 0 {
            f(self.node_xy_to_idx(ux - 1, uy));
        }
        if ux < self.width {
            f(self.node_xy_to_idx(ux + 1, uy));
        }
        if uy > 0 {
            f(self.node_xy_to_idx(ux, uy - 1));
        }
        if uy < self.height {
            f(self.node_xy_to_idx(ux, uy + 1));
        }
    }
}

// Safety: WitnessGraph is immutable after construction and contains no
// interior mutability, so it's safe to share across threads.
unsafe impl Sync for WitnessGraph {}
unsafe impl Send for WitnessGraph {}
