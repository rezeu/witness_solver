use std::collections::HashSet;
use std::fs;

pub use crate::witness::constraints::CellConstraint;
use crate::witness::indexing;
pub use crate::witness::schema::{
    ColoredDotJson, ColoredEdgeDotJson, PuzzleJson, SquareJson, StarJson, SunJson, SymmetryKind,
    TetrisJson, TriangleJson,
};
use crate::witness::types::{EdgeId, NodeId};

// ---------------------------------------------------------------------------
// GraphError
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum GraphError {
    Io(std::io::Error),
    MissingStart,
    MissingEnd,
    InvalidJson(serde_json::Error),
    InvalidPuzzle(String),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::Io(e) => write!(f, "IO error: {}", e),
            GraphError::MissingStart => write!(f, "need at least one start"),
            GraphError::MissingEnd => write!(f, "need at least one end"),
            GraphError::InvalidJson(e) => write!(f, "invalid JSON: {}", e),
            GraphError::InvalidPuzzle(e) => write!(f, "invalid puzzle: {}", e),
        }
    }
}

impl std::error::Error for GraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GraphError::Io(e) => Some(e),
            GraphError::InvalidJson(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Processed graph
// ---------------------------------------------------------------------------

pub struct WitnessGraph {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) start: NodeId,                           // node index
    pub(crate) end: NodeId,                             // node index
    pub(crate) broken: Vec<u64>,                        // bitset of broken edge indices
    pub(crate) dot_nodes: Vec<NodeId>, // list of node indices with black dots (must be visited)
    pub(crate) dot_edges: Vec<EdgeId>, // list of edge indices with black dots (must be used)
    pub(crate) colored_dot_nodes: Vec<(NodeId, u8)>, // (node_index, color), color > 0. Same-color nodes must be in the same region.
    pub(crate) colored_dot_edges: Vec<(EdgeId, u8)>, // (edge_index, color), color > 0. Same-color edges must be in the same region.
    pub(crate) cells: Vec<CellConstraint>,           // cy * width + cx
    pub(crate) has_region_rules: bool, // any cell has square/star/sun/tetris/elimination
    pub(crate) triangle_cells: Vec<(usize, usize, u8)>, // (cx, cy, count) for early pruning
    pub(crate) sun_cells: Vec<(usize, usize, u8)>, // (cx, cy, color) for sun constraints
    /// Pre-computed adjacency: adj[node] = ([neighbor; 4], count).
    /// Avoids division/modulo in hot paths (gen_moves, pruner BFS).
    pub(crate) adj: Vec<([NodeId; 4], u8)>,
    pub(crate) symmetry: Option<SymmetryKind>,
    pub(crate) end_nodes: Vec<NodeId>, // cached result of all_end_nodes()
    pub(crate) expected_degree: Vec<u8>, // cached: 0=none, 1=endpoint, 2=double-role endpoint
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
    pub fn from_file(path: &str) -> Result<Self, GraphError> {
        let text = fs::read_to_string(path).map_err(GraphError::Io)?;
        let json: PuzzleJson = serde_json::from_str(&text).map_err(GraphError::InvalidJson)?;
        Self::from_json(json)
    }

    pub fn from_json(j: PuzzleJson) -> Result<Self, GraphError> {
        Self::validate_json(&j)?;

        let w = j.width;
        let h = j.height;

        if j.starts.is_empty() {
            return Err(GraphError::MissingStart);
        }
        if j.ends.is_empty() {
            return Err(GraphError::MissingEnd);
        }

        let start_xy = j.starts[0];
        let end_xy = j.ends[0];

        let start = Self::xy_to_idx_static(w, start_xy[0], start_xy[1]);
        let end = Self::xy_to_idx_static(w, end_xy[0], end_xy[1]);

        let num_slots = Self::num_edge_slots_static(w, h);
        let bitset_words = num_slots.div_ceil(64);

        // Broken edges
        let mut broken = vec![0u64; bitset_words];
        for pair in &j.broken_edges {
            let u = Self::xy_to_idx_static(w, pair[0][0], pair[0][1]);
            let v = Self::xy_to_idx_static(w, pair[1][0], pair[1][1]);
            let ei = Self::edge_endpoints_to_idx_static(w, u, v);
            set_bit(&mut broken, ei);
        }

        // Dot nodes
        let dot_nodes: Vec<NodeId> = j
            .node_dots
            .iter()
            .map(|xy| Self::xy_to_idx_static(w, xy[0], xy[1]))
            .collect();

        // Dot edges
        let dot_edges: Vec<EdgeId> = j
            .edge_dots
            .iter()
            .map(|pair| {
                let u = Self::xy_to_idx_static(w, pair[0][0], pair[0][1]);
                let v = Self::xy_to_idx_static(w, pair[1][0], pair[1][1]);
                Self::edge_endpoints_to_idx_static(w, u, v)
            })
            .collect();

        // Colored dot nodes (color > 0: same-color nodes must be in the same region)
        let colored_dot_nodes: Vec<(NodeId, u8)> = j
            .colored_node_dots
            .iter()
            .map(|cd| (Self::xy_to_idx_static(w, cd.pos[0], cd.pos[1]), cd.color))
            .collect();

        // Colored dot edges
        let colored_dot_edges: Vec<(EdgeId, u8)> = j
            .colored_edge_dots
            .iter()
            .map(|cd| {
                let u = Self::xy_to_idx_static(w, cd.endpoints[0][0], cd.endpoints[0][1]);
                let v = Self::xy_to_idx_static(w, cd.endpoints[1][0], cd.endpoints[1][1]);
                let ei = Self::edge_endpoints_to_idx_static(w, u, v);
                (ei, cd.color)
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
                can_rotate: te.can_rotate,
            };
        }
        for su in &j.sun_cells {
            let idx = su.pos[1] * w + su.pos[0];
            cells[idx] = CellConstraint::Sun { color: su.color };
        }
        for el in &j.eliminations {
            let idx = el[1] * w + el[0];
            cells[idx] = CellConstraint::Elimination;
        }

        let has_region_rules = cells.iter().any(|c| {
            matches!(
                c,
                CellConstraint::Square { .. }
                    | CellConstraint::Star { .. }
                    | CellConstraint::Sun { .. }
                    | CellConstraint::Tetris { .. }
                    | CellConstraint::Elimination
            )
        });

        // Pre-compute triangle cells for early pruning
        let mut triangle_cells = Vec::new();
        let mut sun_cells = Vec::new();
        for cy in 0..h {
            for cx in 0..w {
                match &cells[cy * w + cx] {
                    CellConstraint::Triangle { count } => {
                        triangle_cells.push((cx, cy, *count));
                    }
                    CellConstraint::Sun { color } => {
                        sun_cells.push((cx, cy, *color));
                    }
                    _ => {}
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

        // Pre-compute end nodes and expected degrees
        let mut end_nodes = vec![start, end];
        if let Some(kind) = j.symmetry {
            let (sx, sy) = (start_xy[0], start_xy[1]);
            let mirror_start = match kind {
                SymmetryKind::MirrorX => {
                    if 2 * sx == w {
                        None
                    } else {
                        Some(Self::xy_to_idx_static(w, w - sx, sy))
                    }
                }
                SymmetryKind::MirrorY => {
                    if 2 * sy == h {
                        None
                    } else {
                        Some(Self::xy_to_idx_static(w, sx, h - sy))
                    }
                }
                SymmetryKind::MirrorXY => {
                    if 2 * sx == w && 2 * sy == h {
                        None
                    } else {
                        Some(Self::xy_to_idx_static(w, w - sx, h - sy))
                    }
                }
            };
            match mirror_start {
                Some(ms) => {
                    if !end_nodes.contains(&ms) {
                        end_nodes.push(ms);
                    }
                }
                None => {
                    end_nodes.push(start);
                }
            }

            let (ex, ey) = (end_xy[0], end_xy[1]);
            let mirror_end = match kind {
                SymmetryKind::MirrorX => {
                    if 2 * ex == w {
                        None
                    } else {
                        Some(Self::xy_to_idx_static(w, w - ex, ey))
                    }
                }
                SymmetryKind::MirrorY => {
                    if 2 * ey == h {
                        None
                    } else {
                        Some(Self::xy_to_idx_static(w, ex, h - ey))
                    }
                }
                SymmetryKind::MirrorXY => {
                    if 2 * ex == w && 2 * ey == h {
                        None
                    } else {
                        Some(Self::xy_to_idx_static(w, w - ex, h - ey))
                    }
                }
            };
            match mirror_end {
                Some(me) => {
                    if !end_nodes.contains(&me) {
                        end_nodes.push(me);
                    }
                }
                None => {
                    end_nodes.push(end);
                }
            }
        }

        let mut expected_degree = vec![0u8; num_nodes];
        for &node in &end_nodes {
            expected_degree[node] += 1;
        }

        Ok(WitnessGraph {
            width: w,
            height: h,
            start,
            end,
            broken,
            dot_nodes,
            dot_edges,
            colored_dot_nodes,
            colored_dot_edges,
            cells,
            has_region_rules,
            triangle_cells,
            sun_cells,
            adj,
            symmetry: j.symmetry,
            end_nodes,
            expected_degree,
        })
    }

    // --- Static helpers (no &self, used during construction) ---------------

    fn validate_json(j: &PuzzleJson) -> Result<(), GraphError> {
        let w = j.width;
        let h = j.height;

        if w == 0 || h == 0 {
            return Err(Self::invalid("width and height must be positive"));
        }

        let node_width = w
            .checked_add(1)
            .ok_or_else(|| Self::invalid("grid width overflows node bounds"))?;
        let node_height = h
            .checked_add(1)
            .ok_or_else(|| Self::invalid("grid height overflows node bounds"))?;
        let num_nodes = node_width
            .checked_mul(node_height)
            .ok_or_else(|| Self::invalid("grid dimensions overflow node count"))?;
        let num_cells = w
            .checked_mul(h)
            .ok_or_else(|| Self::invalid("grid dimensions overflow cell count"))?;
        if num_nodes > 256 || num_cells > 256 {
            return Err(Self::invalid(
                "grid is too large; this solver supports at most 256 nodes and 256 cells",
            ));
        }

        if j.starts.is_empty() {
            return Err(GraphError::MissingStart);
        }
        if j.ends.is_empty() {
            return Err(GraphError::MissingEnd);
        }
        if j.starts.len() > 1 {
            return Err(Self::invalid("multiple start nodes are not supported"));
        }
        if j.ends.len() > 1 {
            return Err(Self::invalid("multiple end nodes are not supported"));
        }

        Self::validate_node_coord(w, h, j.starts[0], "start")?;
        Self::validate_node_coord(w, h, j.ends[0], "end")?;
        if j.starts[0] == j.ends[0] {
            return Err(Self::invalid("start and end must be different nodes"));
        }

        let mut node_constraints = HashSet::new();
        for &xy in &j.node_dots {
            Self::validate_node_coord(w, h, xy, "node dot")?;
            if !node_constraints.insert((xy[0], xy[1])) {
                return Err(Self::invalid(format!(
                    "duplicate node constraint at ({}, {})",
                    xy[0], xy[1]
                )));
            }
        }
        for dot in &j.colored_node_dots {
            Self::validate_colored_dot_color(dot.color, "colored node dot")?;
            Self::validate_node_coord(w, h, dot.pos, "colored node dot")?;
            if !node_constraints.insert((dot.pos[0], dot.pos[1])) {
                return Err(Self::invalid(format!(
                    "duplicate node constraint at ({}, {})",
                    dot.pos[0], dot.pos[1]
                )));
            }
        }

        let mut broken_edges = HashSet::new();
        for pair in &j.broken_edges {
            let ei = Self::validate_edge(w, h, *pair, "broken edge")?;
            if !broken_edges.insert(ei) {
                return Err(Self::invalid(format!("duplicate broken edge {}", ei)));
            }
        }

        let mut edge_constraints = HashSet::new();
        for pair in &j.edge_dots {
            let ei = Self::validate_edge(w, h, *pair, "edge dot")?;
            if broken_edges.contains(&ei) {
                return Err(Self::invalid(format!(
                    "edge dot {} is on a broken edge",
                    ei
                )));
            }
            if !edge_constraints.insert(ei) {
                return Err(Self::invalid(format!("duplicate edge constraint {}", ei)));
            }
        }
        for dot in &j.colored_edge_dots {
            Self::validate_colored_dot_color(dot.color, "colored edge dot")?;
            let ei = Self::validate_edge(w, h, dot.endpoints, "colored edge dot")?;
            if broken_edges.contains(&ei) {
                return Err(Self::invalid(format!(
                    "colored edge dot {} is on a broken edge",
                    ei
                )));
            }
            if !edge_constraints.insert(ei) {
                return Err(Self::invalid(format!("duplicate edge constraint {}", ei)));
            }
        }

        let mut cell_constraints = vec![None; num_cells];
        for sq in &j.squares {
            Self::validate_color(sq.color, "square")?;
            Self::mark_cell_constraint(w, h, &mut cell_constraints, sq.pos, "square")?;
        }
        for st in &j.stars {
            Self::validate_color(st.color, "star")?;
            Self::mark_cell_constraint(w, h, &mut cell_constraints, st.pos, "star")?;
        }
        for tr in &j.triangles {
            if !(1..=3).contains(&tr.count) {
                return Err(Self::invalid(format!(
                    "triangle at ({}, {}) has invalid count {}; expected 1..=3",
                    tr.pos[0], tr.pos[1], tr.count
                )));
            }
            Self::mark_cell_constraint(w, h, &mut cell_constraints, tr.pos, "triangle")?;
        }
        for te in &j.tetris {
            Self::validate_tetris_shape(w, h, te)?;
            Self::mark_cell_constraint(w, h, &mut cell_constraints, te.pos, "tetris")?;
        }
        for su in &j.sun_cells {
            Self::validate_color(su.color, "sun")?;
            Self::mark_cell_constraint(w, h, &mut cell_constraints, su.pos, "sun")?;
        }
        for &pos in &j.eliminations {
            Self::mark_cell_constraint(w, h, &mut cell_constraints, pos, "elimination")?;
        }

        Ok(())
    }

    fn invalid(msg: impl Into<String>) -> GraphError {
        GraphError::InvalidPuzzle(msg.into())
    }

    fn validate_color(color: u8, kind: &str) -> Result<(), GraphError> {
        if color >= 16 {
            return Err(Self::invalid(format!(
                "{} color {} is out of range; expected 0..=15",
                kind, color
            )));
        }
        Ok(())
    }

    fn validate_colored_dot_color(color: u8, kind: &str) -> Result<(), GraphError> {
        if !(1..=15).contains(&color) {
            return Err(Self::invalid(format!(
                "{} color {} is out of range; expected 1..=15",
                kind, color
            )));
        }
        Ok(())
    }

    fn validate_node_coord(
        w: usize,
        h: usize,
        xy: [usize; 2],
        label: &str,
    ) -> Result<(), GraphError> {
        if xy[0] > w || xy[1] > h {
            return Err(Self::invalid(format!(
                "{} coordinate ({}, {}) is outside node bounds 0..={} x 0..={}",
                label, xy[0], xy[1], w, h
            )));
        }
        Ok(())
    }

    fn validate_cell_coord(
        w: usize,
        h: usize,
        xy: [usize; 2],
        label: &str,
    ) -> Result<(), GraphError> {
        if xy[0] >= w || xy[1] >= h {
            return Err(Self::invalid(format!(
                "{} coordinate ({}, {}) is outside cell bounds 0..{} x 0..{}",
                label, xy[0], xy[1], w, h
            )));
        }
        Ok(())
    }

    fn validate_edge(
        w: usize,
        h: usize,
        pair: [[usize; 2]; 2],
        label: &str,
    ) -> Result<usize, GraphError> {
        Self::validate_node_coord(w, h, pair[0], label)?;
        Self::validate_node_coord(w, h, pair[1], label)?;

        let dx = pair[0][0].abs_diff(pair[1][0]);
        let dy = pair[0][1].abs_diff(pair[1][1]);
        if dx + dy != 1 {
            return Err(Self::invalid(format!(
                "{} endpoints ({}, {}) and ({}, {}) must be adjacent grid nodes",
                label, pair[0][0], pair[0][1], pair[1][0], pair[1][1]
            )));
        }

        let u = Self::xy_to_idx_static(w, pair[0][0], pair[0][1]);
        let v = Self::xy_to_idx_static(w, pair[1][0], pair[1][1]);
        Ok(Self::edge_endpoints_to_idx_static(w, u, v))
    }

    fn mark_cell_constraint(
        w: usize,
        h: usize,
        cells: &mut [Option<&'static str>],
        pos: [usize; 2],
        kind: &'static str,
    ) -> Result<(), GraphError> {
        Self::validate_cell_coord(w, h, pos, kind)?;
        let idx = pos[1] * w + pos[0];
        if let Some(existing) = cells[idx] {
            return Err(Self::invalid(format!(
                "cell ({}, {}) has both {} and {} constraints",
                pos[0], pos[1], existing, kind
            )));
        }
        cells[idx] = Some(kind);
        Ok(())
    }

    fn validate_tetris_shape(w: usize, h: usize, te: &TetrisJson) -> Result<(), GraphError> {
        if te.shape.is_empty() {
            return Err(Self::invalid(format!(
                "tetris at ({}, {}) has an empty shape",
                te.pos[0], te.pos[1]
            )));
        }

        let mut offsets = HashSet::new();
        let mut min_x = i16::MAX;
        let mut max_x = i16::MIN;
        let mut min_y = i16::MAX;
        let mut max_y = i16::MIN;
        for &[dx, dy] in &te.shape {
            let dx = dx as i16;
            let dy = dy as i16;
            if !offsets.insert((dx, dy)) {
                return Err(Self::invalid(format!(
                    "tetris at ({}, {}) has duplicate offset ({}, {})",
                    te.pos[0], te.pos[1], dx, dy
                )));
            }
            min_x = min_x.min(dx);
            max_x = max_x.max(dx);
            min_y = min_y.min(dy);
            max_y = max_y.max(dy);
        }

        if !offsets.contains(&(0, 0)) {
            return Err(Self::invalid(format!(
                "tetris at ({}, {}) must include origin offset (0, 0)",
                te.pos[0], te.pos[1]
            )));
        }

        if te.shape.len() > w * h {
            return Err(Self::invalid(format!(
                "tetris at ({}, {}) has area {} larger than the puzzle area {}",
                te.pos[0],
                te.pos[1],
                te.shape.len(),
                w * h
            )));
        }

        if !Self::shape_is_connected(&offsets) {
            return Err(Self::invalid(format!(
                "tetris at ({}, {}) must be 4-neighbor connected",
                te.pos[0], te.pos[1]
            )));
        }

        let shape_w = (max_x - min_x + 1) as usize;
        let shape_h = (max_y - min_y + 1) as usize;
        let fits = shape_w <= w && shape_h <= h;
        let fits_rotated = te.can_rotate && shape_w <= h && shape_h <= w;
        if !fits && !fits_rotated {
            return Err(Self::invalid(format!(
                "tetris at ({}, {}) has bounding box {}x{}, which does not fit {}x{} puzzle",
                te.pos[0], te.pos[1], shape_w, shape_h, w, h
            )));
        }

        Ok(())
    }

    fn shape_is_connected(offsets: &HashSet<(i16, i16)>) -> bool {
        let Some(&first) = offsets.iter().next() else {
            return false;
        };
        let mut seen = HashSet::new();
        let mut stack = vec![first];
        while let Some((x, y)) = stack.pop() {
            if !seen.insert((x, y)) {
                continue;
            }
            for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if offsets.contains(&next) && !seen.contains(&next) {
                    stack.push(next);
                }
            }
        }
        seen.len() == offsets.len()
    }

    fn xy_to_idx_static(w: usize, x: usize, y: usize) -> NodeId {
        indexing::node_xy_to_idx(w, x, y)
    }

    fn num_edge_slots_static(w: usize, h: usize) -> usize {
        indexing::num_edge_slots(w, h)
    }

    fn edge_endpoints_to_idx_static(w: usize, u: NodeId, v: NodeId) -> EdgeId {
        indexing::edge_endpoints_to_idx(w, u, v)
    }

    // --- Instance methods -------------------------------------------------

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    #[inline]
    pub fn start(&self) -> NodeId {
        self.start
    }

    #[inline]
    pub fn end(&self) -> NodeId {
        self.end
    }

    #[inline]
    pub fn symmetry(&self) -> Option<SymmetryKind> {
        self.symmetry
    }

    #[inline]
    pub fn dot_nodes(&self) -> &[NodeId] {
        &self.dot_nodes
    }

    #[inline]
    pub fn dot_edges(&self) -> &[EdgeId] {
        &self.dot_edges
    }

    #[inline]
    pub fn colored_dot_nodes(&self) -> &[(NodeId, u8)] {
        &self.colored_dot_nodes
    }

    #[inline]
    pub fn colored_dot_edges(&self) -> &[(EdgeId, u8)] {
        &self.colored_dot_edges
    }

    #[inline]
    pub fn cells(&self) -> &[CellConstraint] {
        &self.cells
    }

    #[inline]
    pub fn has_region_rules(&self) -> bool {
        self.has_region_rules
    }

    #[inline]
    pub fn triangle_cells(&self) -> &[(usize, usize, u8)] {
        &self.triangle_cells
    }

    #[inline]
    pub fn sun_cells(&self) -> &[(usize, usize, u8)] {
        &self.sun_cells
    }

    #[inline]
    pub fn expected_degree(&self) -> &[u8] {
        &self.expected_degree
    }

    #[inline]
    pub fn num_nodes(&self) -> usize {
        (self.width + 1) * (self.height + 1)
    }

    #[inline]
    pub fn num_edge_slots(&self) -> usize {
        Self::num_edge_slots_static(self.width, self.height)
    }

    #[inline]
    pub fn node_xy_to_idx(&self, x: usize, y: usize) -> NodeId {
        indexing::node_xy_to_idx(self.width, x, y)
    }

    #[inline]
    pub fn node_idx_to_xy(&self, ni: NodeId) -> (usize, usize) {
        indexing::node_idx_to_xy(self.width, ni)
    }

    #[inline]
    pub fn h_edge_index(&self, x: usize, y: usize) -> EdgeId {
        indexing::h_edge_index(self.width, x, y)
    }

    #[inline]
    pub fn v_edge_index(&self, x: usize, y: usize) -> EdgeId {
        indexing::v_edge_index(self.width, x, y)
    }

    pub fn edge_idx_to_endpoints(&self, ei: EdgeId) -> (NodeId, NodeId) {
        indexing::edge_idx_to_endpoints(self.width, ei)
    }

    #[inline]
    pub fn edge_endpoints_to_idx(&self, u: NodeId, v: NodeId) -> EdgeId {
        Self::edge_endpoints_to_idx_static(self.width, u, v)
    }

    #[inline]
    pub fn is_broken(&self, ei: EdgeId) -> bool {
        test_bit(&self.broken, ei)
    }

    #[inline]
    pub fn cell(&self, cx: usize, cy: usize) -> &CellConstraint {
        &self.cells[cy * self.width + cx]
    }

    /// Iterate all grid-adjacent nodes of `u` via closure (zero-allocation).
    /// Uses pre-computed adjacency list — no division/modulo.
    #[inline]
    pub fn for_each_neighbor(&self, u: NodeId, mut f: impl FnMut(NodeId)) {
        let (neighbors, count) = &self.adj[u];
        for &n in neighbors.iter().take(*count as usize) {
            f(n);
        }
    }
    /// If the puzzle has symmetry, return the mirror node for `ni`.
    /// Returns `None` when the node is on the symmetry axis (self-symmetric).
    pub fn symmetric_node(&self, ni: NodeId) -> Option<NodeId> {
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
    pub fn symmetric_edge(&self, ei: EdgeId) -> Option<EdgeId> {
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
    pub fn all_end_nodes(&self) -> Vec<NodeId> {
        self.end_nodes.clone()
    }

    pub fn end_nodes_ref(&self) -> &[NodeId] {
        &self.end_nodes
    }
}

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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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
                assert_ne!(
                    h, v,
                    "h_edge({},{})={} collides with v_edge({},{})={}",
                    x, y, h, x, y, v
                );
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
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(0, 0)).unwrap(),
            g.node_xy_to_idx(4, 0)
        );
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(4, 0)).unwrap(),
            g.node_xy_to_idx(0, 0)
        );
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(1, 3)).unwrap(),
            g.node_xy_to_idx(3, 3)
        );
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(3, 3)).unwrap(),
            g.node_xy_to_idx(1, 3)
        );
        // On-axis nodes (x=2) return None
        assert!(g.symmetric_node(g.node_xy_to_idx(2, 0)).is_none());
        assert!(g.symmetric_node(g.node_xy_to_idx(2, 4)).is_none());
    }

    #[test]
    fn test_symmetric_node_mirror_y() {
        let g = make_symmetry_graph(SymmetryKind::MirrorY, 4, 4);
        // Off-axis nodes mirror across y=2
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(0, 0)).unwrap(),
            g.node_xy_to_idx(0, 4)
        );
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(0, 4)).unwrap(),
            g.node_xy_to_idx(0, 0)
        );
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(3, 1)).unwrap(),
            g.node_xy_to_idx(3, 3)
        );
        // On-axis nodes (y=2) return None
        assert!(g.symmetric_node(g.node_xy_to_idx(0, 2)).is_none());
        assert!(g.symmetric_node(g.node_xy_to_idx(4, 2)).is_none());
    }

    #[test]
    fn test_symmetric_node_mirror_xy() {
        let g = make_symmetry_graph(SymmetryKind::MirrorXY, 4, 4);
        // Off-axis nodes mirror across (x=2, y=2)
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(0, 0)).unwrap(),
            g.node_xy_to_idx(4, 4)
        );
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(4, 4)).unwrap(),
            g.node_xy_to_idx(0, 0)
        );
        assert_eq!(
            g.symmetric_node(g.node_xy_to_idx(3, 1)).unwrap(),
            g.node_xy_to_idx(1, 3)
        );
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
                    assert_eq!(
                        g.symmetric_edge(m).unwrap(),
                        he,
                        "double mirror of h_edge({},{}) should be itself",
                        x,
                        y
                    );
                }
                let ve = g.v_edge_index(x, y);
                if let Some(m) = g.symmetric_edge(ve) {
                    assert_eq!(
                        g.symmetric_edge(m).unwrap(),
                        ve,
                        "double mirror of v_edge({},{}) should be itself",
                        x,
                        y
                    );
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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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
                assert!(
                    matches!(cell, CellConstraint::None),
                    "cell({},{}) should be None, got {:?}",
                    x,
                    y,
                    cell
                );
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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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

        let jnone: PuzzleJson =
            serde_json::from_str(r#"{"width":2,"height":2,"starts":[[0,0]],"ends":[[2,2]]}"#)
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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
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
                    g.edge_endpoints_to_idx(u, v),
                    ei,
                    "roundtrip failed for edge {}",
                    ei
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
                    g.edge_endpoints_to_idx(u, v),
                    ei,
                    "h_edge({},{}): roundtrip mismatch",
                    x,
                    y
                );
            }
        }
        for y in 0..4 {
            for x in 0..=4 {
                let ei = g.v_edge_index(x, y);
                let (u, v) = g.edge_idx_to_endpoints(ei);
                assert_eq!(
                    g.edge_endpoints_to_idx(u, v),
                    ei,
                    "v_edge({},{}): roundtrip mismatch",
                    x,
                    y
                );
            }
        }
    }
}
