use crate::solver::Satisfier;
use crate::witness::graph::{CellConstraint, WitnessGraph};
#[cfg(test)]
use crate::witness::graph::{PuzzleJson, SymmetryKind};
use crate::witness::region::{RegionMap, compute_regions};
use crate::witness::state::WitnessState;

type TetrisShape = Vec<(i8, i8)>;
type TetrisRotations = Vec<TetrisShape>;
type TetrisShapeChoice = (TetrisRotations, bool);

/// Single satisfier that checks ALL witness rules in optimal order.
/// Region computation is deferred until needed and shared across rule checks.
pub struct WitnessValidator {
    has_region_rules: bool,
    has_eliminations: bool,
    has_tetris: bool,
    has_suns: bool,
}

impl WitnessValidator {
    pub fn new(g: &WitnessGraph) -> Self {
        let has_eliminations = g
            .cells
            .iter()
            .any(|c| matches!(c, CellConstraint::Elimination));
        let has_tetris = g
            .cells
            .iter()
            .any(|c| matches!(c, CellConstraint::Tetris { .. }));
        let has_suns = g
            .cells
            .iter()
            .any(|c| matches!(c, CellConstraint::Sun { .. }));
        WitnessValidator {
            has_region_rules: g.has_region_rules,
            has_eliminations,
            has_tetris,
            has_suns,
        }
    }
}

impl Satisfier<WitnessState> for WitnessValidator {
    fn is_satisfied(&self, s: &WitnessState, g: &WitnessGraph) -> bool {
        // 1. Path must terminate at end
        if s.head != g.end {
            return false;
        }

        // 2. Degree invariant: start/end degree 1, others 0 or 2
        if !check_degrees(s, g) {
            return false;
        }

        // 3. All dot nodes and edges (black + colored) must be traversed
        if !check_dots(s, g) {
            return false;
        }

        // 4. Non-region rules without elimination
        if !self.has_eliminations && !check_triangles(s, g) {
            return false;
        }

        // 5. Region-based rules
        if self.has_region_rules || (self.has_eliminations && !g.triangle_cells.is_empty()) {
            let regions = compute_regions(s, g);

            if self.has_eliminations {
                // Elimination path: check all constraints per-region with elimination pairing
                return check_regions_with_elimination(
                    s,
                    g,
                    &regions,
                    self.has_tetris,
                    self.has_suns,
                );
            }

            if !check_squares(g, &regions) {
                return false;
            }
            if !check_stars(g, &regions) {
                return false;
            }
            if self.has_suns && !check_suns(g, &regions) {
                return false;
            }
            if self.has_tetris && !check_tetris(g, &regions) {
                return false;
            }
            if (!g.colored_dot_nodes.is_empty() || !g.colored_dot_edges.is_empty())
                && !check_colored_dots_in_regions(g, &regions, s)
            {
                return false;
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// Individual rule checks
// ---------------------------------------------------------------------------

fn check_degrees(s: &WitnessState, g: &WitnessGraph) -> bool {
    s.degrees.iter().zip(&g.expected_degree).all(
        |(&d, &exp)| {
            if exp > 0 { d == exp } else { d == 0 || d == 2 }
        },
    )
}

fn check_dots(s: &WitnessState, g: &WitnessGraph) -> bool {
    for &ni in &g.dot_nodes {
        if s.degrees[ni] == 0 {
            return false;
        }
    }
    for &ei in &g.dot_edges {
        if !s.used(ei) {
            return false;
        }
    }
    for &(ni, _color) in &g.colored_dot_nodes {
        if s.degrees[ni] == 0 {
            return false;
        }
    }
    for &(ei, _color) in &g.colored_dot_edges {
        if !s.used(ei) {
            return false;
        }
    }
    true
}

fn check_triangles(s: &WitnessState, g: &WitnessGraph) -> bool {
    for cy in 0..g.height {
        for cx in 0..g.width {
            if let CellConstraint::Triangle { count } = g.cell(cx, cy) {
                let used = count_boundary_edges(s, g, cx, cy);
                if used != *count as usize {
                    return false;
                }
            }
        }
    }
    true
}

fn count_boundary_edges(s: &WitnessState, g: &WitnessGraph, cx: usize, cy: usize) -> usize {
    let mut n = 0;
    if s.used(g.h_edge_index(cx, cy)) {
        n += 1;
    }
    if s.used(g.h_edge_index(cx, cy + 1)) {
        n += 1;
    }
    if s.used(g.v_edge_index(cx, cy)) {
        n += 1;
    }
    if s.used(g.v_edge_index(cx + 1, cy)) {
        n += 1;
    }
    n
}

/// Each region must contain squares of only one color.
fn check_squares(g: &WitnessGraph, regions: &RegionMap) -> bool {
    for r in 0..regions.count {
        if !check_squares_in_region(g, regions, r) {
            return false;
        }
    }
    true
}

pub(crate) fn check_squares_in_region(g: &WitnessGraph, regions: &RegionMap, r: u8) -> bool {
    let mut seen_color: Option<u8> = None;
    for &(cx, cy) in regions.cells_in_region(r) {
        if let CellConstraint::Square { color } = g.cell(cx, cy) {
            match seen_color {
                None => seen_color = Some(*color),
                Some(c) if c != *color => return false,
                _ => {}
            }
        }
    }
    true
}

/// For each color with at least one star in a region, the total count of
/// elements of that color (stars + squares) must be exactly 2.
fn check_stars(g: &WitnessGraph, regions: &RegionMap) -> bool {
    for r in 0..regions.count {
        if !check_stars_in_region(g, regions, r) {
            return false;
        }
    }
    true
}

pub(crate) fn check_stars_in_region(g: &WitnessGraph, regions: &RegionMap, r: u8) -> bool {
    let mut color_count = [0u8; 16];
    let mut has_star = [false; 16];

    for &(cx, cy) in regions.cells_in_region(r) {
        match g.cell(cx, cy) {
            CellConstraint::Star { color } => {
                let c = *color as usize;
                if c < 16 {
                    has_star[c] = true;
                    color_count[c] += 1;
                }
            }
            CellConstraint::Square { color } => {
                let c = *color as usize;
                if c < 16 {
                    color_count[c] += 1;
                }
            }
            _ => {}
        }
    }

    for c in 0..16 {
        if has_star[c] && color_count[c] != 2 {
            return false;
        }
    }
    true
}

/// In each region, for each color that has at least one sun, the total count
/// of suns of that color must be exactly 2.
fn check_suns(g: &WitnessGraph, regions: &RegionMap) -> bool {
    for r in 0..regions.count {
        if !check_suns_in_region(g, regions, r) {
            return false;
        }
    }
    true
}

pub(crate) fn check_suns_in_region(g: &WitnessGraph, regions: &RegionMap, r: u8) -> bool {
    let mut color_count = [0u8; 16];
    let mut has_sun = [false; 16];

    for &(cx, cy) in regions.cells_in_region(r) {
        if let CellConstraint::Sun { color } = g.cell(cx, cy) {
            let c = *color as usize;
            if c < 16 {
                has_sun[c] = true;
                color_count[c] += 1;
            }
        }
    }

    for c in 0..16 {
        if has_sun[c] && color_count[c] != 2 {
            return false;
        }
    }
    true
}

/// All colored dots (nodes + edges) of the same color must be in the same region.
fn check_colored_dots_in_regions(g: &WitnessGraph, regions: &RegionMap, s: &WitnessState) -> bool {
    use std::collections::HashMap;
    let mut color_to_region: HashMap<u8, u8> = HashMap::new();

    for &(ni, color) in &g.colored_dot_nodes {
        if s.degrees[ni] == 0 {
            return false;
        }
        let (nx, ny) = g.node_idx_to_xy(ni);
        let cx = nx.min(g.width - 1);
        let cy = ny.min(g.height - 1);
        let r = regions.cell_region(cx, cy);
        match color_to_region.get(&color) {
            Some(&existing) if existing != r => return false,
            _ => {
                color_to_region.insert(color, r);
            }
        }
    }

    for &(ei, color) in &g.colored_dot_edges {
        if !s.used(ei) {
            return false;
        }
        let (u, v) = g.edge_idx_to_endpoints(ei);
        let (ux, uy) = g.node_idx_to_xy(u);
        let (vx, vy) = g.node_idx_to_xy(v);
        let cx = usize::min(ux, vx).min(g.width - 1);
        let cy = usize::min(uy, vy).min(g.height - 1);
        let r = regions.cell_region(cx, cy);
        match color_to_region.get(&color) {
            Some(&existing) if existing != r => return false,
            _ => {
                color_to_region.insert(color, r);
            }
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Tetris / polyomino tiling
// ---------------------------------------------------------------------------

/// Check tetris constraints for all regions.
fn check_tetris(g: &WitnessGraph, regions: &RegionMap) -> bool {
    for r in 0..regions.count {
        if !check_tetris_in_region(g, regions, r) {
            return false;
        }
    }
    true
}

/// Generate all unique rotation variants of a shape.
fn all_rotations(shape: &[(i8, i8)]) -> Vec<Vec<(i8, i8)>> {
    let mut result = Vec::new();
    let mut current: Vec<(i8, i8)> = shape.to_vec();

    for _ in 0..4 {
        let min_x = current.iter().map(|p| p.0).min().unwrap_or(0);
        let min_y = current.iter().map(|p| p.1).min().unwrap_or(0);
        let normalized: Vec<(i8, i8)> =
            current.iter().map(|p| (p.0 - min_x, p.1 - min_y)).collect();

        let already_seen = result.iter().any(|existing: &Vec<(i8, i8)>| {
            existing.len() == normalized.len() && normalized.iter().all(|p| existing.contains(p))
        });

        if !already_seen {
            result.push(normalized);
        }

        // Rotate 90° clockwise: (x, y) -> (y, -x)
        current = current.iter().map(|p| (p.1, -p.0)).collect();
    }

    result
}

/// Check tetris constraints in a single region.
///
/// All shapes are placed on the grid by translation and rotation. The net coverage
/// (positive cells +1, negative cells −1) must exactly match the region:
/// region cells = +1, non-region cells = 0.
fn check_tetris_in_region(g: &WitnessGraph, regions: &RegionMap, r: u8) -> bool {
    let cells = regions.cells_in_region(r);

    // Collect shapes: positive first, then negative (placement order matters for pruning)
    let mut pos_shapes: Vec<(Vec<(i8, i8)>, bool)> = Vec::new();
    let mut neg_shapes: Vec<(Vec<(i8, i8)>, bool)> = Vec::new();

    for &(cx, cy) in cells {
        if let CellConstraint::Tetris {
            shape,
            negative,
            can_rotate,
        } = g.cell(cx, cy)
        {
            let min_dx = shape.iter().map(|o| o[0]).min().unwrap_or(0);
            let min_dy = shape.iter().map(|o| o[1]).min().unwrap_or(0);
            let norm: Vec<(i8, i8)> = shape
                .iter()
                .map(|o| (o[0] - min_dx, o[1] - min_dy))
                .collect();
            if *negative {
                neg_shapes.push((norm, *can_rotate));
            } else {
                pos_shapes.push((norm, *can_rotate));
            }
        }
    }

    if pos_shapes.is_empty() && neg_shapes.is_empty() {
        return true;
    }

    // Area check
    let pos_area: usize = pos_shapes.iter().map(|(s, _)| s.len()).sum();
    let neg_area: usize = neg_shapes.iter().map(|(s, _)| s.len()).sum();
    if pos_area < neg_area || pos_area - neg_area != cells.len() {
        return false;
    }

    // Build region membership
    let w = g.width;
    let h = g.height;
    let mut region_mask = vec![false; w * h];
    for &(cx, cy) in cells {
        region_mask[cy * w + cx] = true;
    }

    // Coverage grid: positive shapes add +1, negative shapes add -1.
    // Valid final state: region cells = +1, non-region cells = 0.
    let mut coverage = vec![0i8; w * h];

    // Ordered shapes: positive first, then negative
    let mut all_shapes: Vec<TetrisShapeChoice> = Vec::new();
    for (s, can_rotate) in &pos_shapes {
        let rots = if *can_rotate {
            all_rotations(s)
        } else {
            vec![s.clone()]
        };
        all_shapes.push((rots, false));
    }
    for (s, can_rotate) in &neg_shapes {
        let rots = if *can_rotate {
            all_rotations(s)
        } else {
            vec![s.clone()]
        };
        all_shapes.push((rots, true));
    }

    tile_backtrack(w, h, &all_shapes, 0, &region_mask, &mut coverage)
}

/// Backtracking tiling on the full grid.
fn tile_backtrack(
    w: usize,
    h: usize,
    shapes: &[TetrisShapeChoice],
    idx: usize,
    region: &[bool],
    coverage: &mut [i8],
) -> bool {
    if idx == shapes.len() {
        // Verify: region cells = 1, non-region cells = 0
        for i in 0..w * h {
            let expected = if region[i] { 1 } else { 0 };
            if coverage[i] != expected {
                return false;
            }
        }
        return true;
    }

    let (rotations, negative) = &shapes[idx];
    let delta: i8 = if *negative { -1 } else { 1 };

    // First uncovered cell heuristic: for idx > 0, only try placements covering
    // the first cell that doesn't match the expected coverage.
    if idx > 0 {
        if let Some(target_ci) = (0..w * h).find(|&i| {
            let expected = if region[i] { 1 } else { 0 };
            coverage[i] != expected
        }) {
            let target_cx = (target_ci % w) as i8;
            let target_cy = (target_ci / w) as i8;

            for rotation in rotations.iter() {
                for &(dx, dy) in rotation.iter() {
                    let ox = target_cx - dx;
                    let oy = target_cy - dy;
                    if ox < 0 || oy < 0 || ox >= w as i8 || oy >= h as i8 {
                        continue;
                    }

                    let mut valid = true;
                    let mut placed = Vec::new();

                    for &(dx2, dy2) in rotation.iter() {
                        let cx = ox + dx2;
                        let cy = oy + dy2;
                        if cx < 0 || cy < 0 || cx >= w as i8 || cy >= h as i8 {
                            valid = false;
                            break;
                        }
                        let ci = cy as usize * w + cx as usize;
                        // Pruning: positive cells shouldn't exceed 1, negative shouldn't go below 0
                        let new_val = coverage[ci] + delta;
                        if !(0..=1).contains(&new_val) {
                            valid = false;
                            break;
                        }
                        placed.push(ci);
                    }

                    if !valid {
                        continue;
                    }

                    // Apply placement
                    for &ci in &placed {
                        coverage[ci] += delta;
                    }

                    if tile_backtrack(w, h, shapes, idx + 1, region, coverage) {
                        return true;
                    }

                    // Undo
                    for &ci in &placed {
                        coverage[ci] -= delta;
                    }
                }
            }

            return false;
        }
    }

    for oy in 0..h as i8 {
        for ox in 0..w as i8 {
            for rotation in rotations.iter() {
                let mut valid = true;
                let mut placed = Vec::new();

                for &(dx, dy) in rotation.iter() {
                    let cx = ox + dx;
                    let cy = oy + dy;
                    if cx < 0 || cy < 0 || cx >= w as i8 || cy >= h as i8 {
                        valid = false;
                        break;
                    }
                    let ci = cy as usize * w + cx as usize;
                    // Pruning: positive cells shouldn't exceed 1, negative shouldn't go below 0
                    let new_val = coverage[ci] + delta;
                    if !(0..=1).contains(&new_val) {
                        valid = false;
                        break;
                    }
                    placed.push(ci);
                }

                if !valid {
                    continue;
                }

                // Apply placement
                for &ci in &placed {
                    coverage[ci] += delta;
                }

                if tile_backtrack(w, h, shapes, idx + 1, region, coverage) {
                    return true;
                }

                // Undo
                for &ci in &placed {
                    coverage[ci] -= delta;
                }
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Elimination — cancel constraint violations per-region
// ---------------------------------------------------------------------------

/// Check all constraints with elimination support.
/// For each region: count violations and elimination marks, pair them.
fn check_regions_with_elimination(
    s: &WitnessState,
    g: &WitnessGraph,
    regions: &RegionMap,
    has_tetris: bool,
    has_suns: bool,
) -> bool {
    for r in 0..regions.count {
        let cells = regions.cells_in_region(r);

        // Count elimination symbols in this region
        let elim_count = cells
            .iter()
            .filter(|&&(cx, cy)| matches!(g.cell(cx, cy), CellConstraint::Elimination))
            .count();

        // Collect violations
        let mut violations: usize = 0;

        // Triangle violations in this region
        for &(cx, cy) in cells {
            if let CellConstraint::Triangle { count } = g.cell(cx, cy) {
                let used = count_boundary_edges(s, g, cx, cy);
                if used != *count as usize {
                    violations += 1;
                }
            }
        }

        // Square violations
        if !check_squares_in_region(g, regions, r) {
            // Count distinct color conflicts: each mismatched square is a violation
            let mut colors_seen = Vec::new();
            for &(cx, cy) in cells {
                if let CellConstraint::Square { color } = g.cell(cx, cy) {
                    if !colors_seen.contains(color) {
                        colors_seen.push(*color);
                    }
                }
            }
            // Each extra color beyond the first is a violation source.
            // But elimination removes a SYMBOL, so each mismatched square is one violation.
            // Simplification: count squares whose color differs from the majority.
            if colors_seen.len() > 1 {
                // Each square with a minority color is a violation
                let mut color_counts = [0u8; 16];
                for &(cx, cy) in cells {
                    if let CellConstraint::Square { color } = g.cell(cx, cy) {
                        color_counts[*color as usize] += 1;
                    }
                }
                let max_count = *color_counts.iter().max().unwrap_or(&0);
                let total_squares: u8 = color_counts.iter().sum();
                violations += (total_squares - max_count) as usize;
            }
        }

        // Star violations
        if !check_stars_in_region(g, regions, r) {
            // Count star symbols that cause violations
            let mut color_count = [0u8; 16];
            let mut has_star = [false; 16];
            for &(cx, cy) in cells {
                match g.cell(cx, cy) {
                    CellConstraint::Star { color } => {
                        has_star[*color as usize] = true;
                        color_count[*color as usize] += 1;
                    }
                    CellConstraint::Square { color } => {
                        color_count[*color as usize] += 1;
                    }
                    _ => {}
                }
            }
            for c in 0..16 {
                if has_star[c] && color_count[c] != 2 {
                    // Each excess or deficit element is a violation
                    if color_count[c] > 2 {
                        violations += (color_count[c] - 2) as usize;
                    } else {
                        // Too few — the star itself is the violation
                        violations += 1;
                    }
                }
            }
        }

        // Sun violations
        if has_suns && !check_suns_in_region(g, regions, r) {
            let mut color_count = [0u8; 16];
            let mut has_sun = [false; 16];
            for &(cx, cy) in cells {
                if let CellConstraint::Sun { color } = g.cell(cx, cy) {
                    let c = *color as usize;
                    if c < 16 {
                        has_sun[c] = true;
                        color_count[c] += 1;
                    }
                }
            }
            for c in 0..16 {
                if has_sun[c] && color_count[c] != 2 {
                    if color_count[c] > 2 {
                        violations += (color_count[c] - 2) as usize;
                    } else {
                        violations += 1;
                    }
                }
            }
        }

        // Tetris violations
        if has_tetris && !check_tetris_in_region(g, regions, r) {
            // Tetris failure in this region — count tetris symbols as violations
            let tetris_count = cells
                .iter()
                .filter(|&&(cx, cy)| matches!(g.cell(cx, cy), CellConstraint::Tetris { .. }))
                .count();
            violations += tetris_count;
        }

        // Pair violations with eliminations
        // Rule: each elimination cancels one symbol. Excess eliminations pair among themselves.
        if violations > elim_count {
            return false; // Not enough eliminations to cancel all violations
        }
        let excess_elim = elim_count - violations;
        if excess_elim % 2 != 0 {
            return false; // Unpaired elimination (can't cancel itself alone)
        }
    }

    if (!g.colored_dot_nodes.is_empty() || !g.colored_dot_edges.is_empty())
        && !check_colored_dots_in_regions(g, regions, s)
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::witness::graph::{ColoredDotJson, SunJson};
    use crate::witness::state::WitnessState;

    fn make_graph(w: usize, h: usize) -> WitnessGraph {
        let json = PuzzleJson {
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
        };
        WitnessGraph::from_json(json).unwrap()
    }

    fn make_symmetry_graph() -> WitnessGraph {
        let json = PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[4, 4]],
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

    fn make_dot_graph() -> WitnessGraph {
        let json = PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[4, 4]],
            symmetry: None,
            node_dots: vec![[2, 2]],
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
    fn check_degrees_valid() {
        let g = make_graph(4, 4);
        let mut s = WitnessState::new(&g);
        s.degrees[g.start] = 1;
        s.degrees[g.end] = 1;
        assert!(super::check_degrees(&s, &g));
    }

    #[test]
    fn check_degrees_start_degree_2_fails() {
        let g = make_graph(4, 4);
        let mut s = WitnessState::new(&g);
        s.degrees[g.start] = 2;
        s.degrees[g.end] = 1;
        assert!(!super::check_degrees(&s, &g));
    }

    #[test]
    fn check_degrees_unknown_degree_1_fails() {
        let g = make_graph(4, 4);
        let mut s = WitnessState::new(&g);
        s.degrees[g.start] = 1;
        s.degrees[g.end] = 1;
        let mid = g.node_xy_to_idx(1, 1);
        s.degrees[mid] = 1;
        assert!(!super::check_degrees(&s, &g));
    }

    #[test]
    fn check_dots_all_visited() {
        let g = make_dot_graph();
        let mut s = WitnessState::new(&g);
        let dot = g.node_xy_to_idx(2, 2);
        s.degrees[dot] = 2;
        assert!(super::check_dots(&s, &g));
    }

    #[test]
    fn check_dots_unreachable() {
        let g = make_dot_graph();
        let s = WitnessState::new(&g);
        assert!(!super::check_dots(&s, &g));
    }

    #[test]
    fn check_degrees_symmetry_4_endpoints() {
        let g = make_symmetry_graph();
        let mut s = WitnessState::new(&g);
        let ends = g.all_end_nodes();
        for &ni in &ends {
            s.degrees[ni] = 1;
        }
        assert!(super::check_degrees(&s, &g));
    }

    #[test]
    fn check_degrees_symmetry_mirror_end_missing() {
        let g = make_symmetry_graph();
        let mut s = WitnessState::new(&g);
        let ends = g.all_end_nodes();
        for (i, &ni) in ends.iter().enumerate() {
            s.degrees[ni] = if i < ends.len() - 1 { 1 } else { 0 };
        }
        assert!(!super::check_degrees(&s, &g));
    }

    fn make_sun_graph(suns: Vec<SunJson>) -> WitnessGraph {
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
            sun_cells: suns,
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
        };
        WitnessGraph::from_json(json).unwrap()
    }

    fn set_used(state: &mut WitnessState, ei: usize) {
        let w = ei >> 6;
        let b = ei & 63;
        state.used_edges[w] |= 1u64 << b;
    }

    #[test]
    fn check_suns_in_region_pass() {
        let g = make_sun_graph(vec![
            SunJson {
                pos: [0, 0],
                color: 1,
            },
            SunJson {
                pos: [1, 0],
                color: 1,
            },
        ]);
        let s = WitnessState::new(&g);
        let regions = compute_regions(&s, &g);
        assert!(super::check_suns_in_region(&g, &regions, 0));
    }

    #[test]
    fn check_suns_in_region_fail() {
        let g = make_sun_graph(vec![SunJson {
            pos: [0, 0],
            color: 1,
        }]);
        let s = WitnessState::new(&g);
        let regions = compute_regions(&s, &g);
        assert!(!super::check_suns_in_region(&g, &regions, 0));
    }

    fn make_colored_dot_graph(dots: Vec<ColoredDotJson>) -> WitnessGraph {
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
            colored_node_dots: dots,
            colored_edge_dots: vec![],
        };
        WitnessGraph::from_json(json).unwrap()
    }

    #[test]
    fn check_colored_dots_same_region_pass() {
        let g = make_colored_dot_graph(vec![
            ColoredDotJson {
                pos: [0, 0],
                color: 1,
            },
            ColoredDotJson {
                pos: [2, 2],
                color: 1,
            },
        ]);
        let mut s = WitnessState::new(&g);
        let n1 = g.node_xy_to_idx(0, 0);
        let n2 = g.node_xy_to_idx(2, 2);
        s.degrees[n1] = 2;
        s.degrees[n2] = 2;
        let regions = compute_regions(&s, &g);
        assert!(super::check_colored_dots_in_regions(&g, &regions, &s));
    }

    #[test]
    fn check_colored_dots_different_regions_fail() {
        let g = make_colored_dot_graph(vec![
            ColoredDotJson {
                pos: [0, 0],
                color: 1,
            },
            ColoredDotJson {
                pos: [2, 0],
                color: 1,
            },
        ]);
        let mut s = WitnessState::new(&g);
        set_used(&mut s, g.v_edge_index(1, 0));
        set_used(&mut s, g.v_edge_index(1, 1));
        let n1 = g.node_xy_to_idx(0, 0);
        let n2 = g.node_xy_to_idx(2, 0);
        s.degrees[n1] = 2;
        s.degrees[n2] = 2;
        let regions = compute_regions(&s, &g);
        assert!(!super::check_colored_dots_in_regions(&g, &regions, &s));
    }
}
