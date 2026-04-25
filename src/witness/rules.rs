use crate::solver::Satisfier;
use crate::witness::graph::{CellConstraint, WitnessGraph};
use crate::witness::region::{compute_regions, RegionMap};
use crate::witness::state::WitnessState;

/// Single satisfier that checks ALL witness rules in optimal order.
/// Region computation is deferred until needed and shared across rule checks.
pub struct WitnessValidator {
    has_region_rules: bool,
    has_eliminations: bool,
    has_tetris: bool,
}

impl WitnessValidator {
    pub fn new(g: &WitnessGraph) -> Self {
        let has_eliminations = g.cells.iter().any(|c| matches!(c, CellConstraint::Elimination));
        let has_tetris = g.cells.iter().any(|c| matches!(c, CellConstraint::Tetris { .. }));
        WitnessValidator {
            has_region_rules: g.has_region_rules,
            has_eliminations,
            has_tetris,
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

        // 3. All dot nodes and edges must be traversed
        if !check_dots(s, g) {
            return false;
        }

        // 4. Non-region rules without elimination
        if !self.has_eliminations {
            if !check_triangles(s, g) {
                return false;
            }
        }

        // 5. Region-based rules
        if self.has_region_rules || (self.has_eliminations && !g.triangle_cells.is_empty()) {
            let regions = compute_regions(s, g);

            if self.has_eliminations {
                // Elimination path: check all constraints per-region with elimination pairing
                return check_regions_with_elimination(s, g, &regions, self.has_tetris);
            }

            if !check_squares(g, &regions) {
                return false;
            }
            if !check_stars(g, &regions) {
                return false;
            }
            if self.has_tetris {
                if !check_tetris(g, &regions) {
                    return false;
                }
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// Individual rule checks
// ---------------------------------------------------------------------------

fn check_degrees(s: &WitnessState, g: &WitnessGraph) -> bool {
    for (i, &d) in s.degrees.iter().enumerate() {
        if i == g.start || i == g.end {
            if d != 1 {
                return false;
            }
        } else if d != 0 && d != 2 {
            return false;
        }
    }
    true
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
    for (cx, cy) in regions.cells_in_region(r) {
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

    for (cx, cy) in regions.cells_in_region(r) {
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

/// Check tetris constraints in a single region.
///
/// All shapes are placed on the grid by translation. The net coverage
/// (positive cells +1, negative cells −1) must exactly match the region:
/// region cells = +1, non-region cells = 0.
fn check_tetris_in_region(g: &WitnessGraph, regions: &RegionMap, r: u8) -> bool {
    let cells: Vec<(usize, usize)> = regions.cells_in_region(r).collect();

    // Collect shapes: positive first, then negative (placement order matters for pruning)
    let mut pos_shapes: Vec<Vec<(i8, i8)>> = Vec::new();
    let mut neg_shapes: Vec<Vec<(i8, i8)>> = Vec::new();

    for &(cx, cy) in &cells {
        if let CellConstraint::Tetris { shape, negative } = g.cell(cx, cy) {
            let min_dx = shape.iter().map(|o| o[0]).min().unwrap_or(0);
            let min_dy = shape.iter().map(|o| o[1]).min().unwrap_or(0);
            let norm: Vec<(i8, i8)> = shape.iter().map(|o| (o[0] - min_dx, o[1] - min_dy)).collect();
            if *negative {
                neg_shapes.push(norm);
            } else {
                pos_shapes.push(norm);
            }
        }
    }

    if pos_shapes.is_empty() && neg_shapes.is_empty() {
        return true;
    }

    // Area check
    let pos_area: usize = pos_shapes.iter().map(|s| s.len()).sum();
    let neg_area: usize = neg_shapes.iter().map(|s| s.len()).sum();
    if pos_area < neg_area || pos_area - neg_area != cells.len() {
        return false;
    }

    // Build region membership
    let w = g.width;
    let h = g.height;
    let mut region_mask = vec![false; w * h];
    for &(cx, cy) in &cells {
        region_mask[cy * w + cx] = true;
    }

    // Coverage grid: positive shapes add +1, negative shapes add -1.
    // Valid final state: region cells = +1, non-region cells = 0.
    let mut coverage = vec![0i8; w * h];

    // Ordered shapes: positive first, then negative
    let mut all_shapes: Vec<(&Vec<(i8, i8)>, bool)> = Vec::new();
    for s in &pos_shapes {
        all_shapes.push((s, false));
    }
    for s in &neg_shapes {
        all_shapes.push((s, true));
    }

    tile_backtrack(w, h, &all_shapes, 0, &region_mask, &mut coverage)
}

/// Backtracking tiling on the full grid.
fn tile_backtrack(
    w: usize,
    h: usize,
    shapes: &[(&Vec<(i8, i8)>, bool)],
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

    let (shape, negative) = &shapes[idx];
    let delta: i8 = if *negative { -1 } else { 1 };

    for oy in 0..h as i8 {
        for ox in 0..w as i8 {
            let mut valid = true;
            let mut placed = Vec::new();

            for &(dx, dy) in shape.iter() {
                let cx = ox + dx;
                let cy = oy + dy;
                if cx < 0 || cy < 0 || cx >= w as i8 || cy >= h as i8 {
                    valid = false;
                    break;
                }
                let ci = cy as usize * w + cx as usize;
                // Pruning: positive cells shouldn't exceed 1, negative shouldn't go below 0
                let new_val = coverage[ci] + delta;
                if new_val < 0 || new_val > 1 {
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
) -> bool {
    for r in 0..regions.count {
        let cells: Vec<(usize, usize)> = regions.cells_in_region(r).collect();

        // Count elimination symbols in this region
        let elim_count = cells.iter()
            .filter(|&&(cx, cy)| matches!(g.cell(cx, cy), CellConstraint::Elimination))
            .count();

        // Collect violations
        let mut violations: usize = 0;

        // Triangle violations in this region
        for &(cx, cy) in &cells {
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
            for (cx, cy) in regions.cells_in_region(r) {
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
                for (cx, cy) in regions.cells_in_region(r) {
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
            for (cx, cy) in regions.cells_in_region(r) {
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

        // Tetris violations
        if has_tetris && !check_tetris_in_region(g, regions, r) {
            // Tetris failure in this region — count tetris symbols as violations
            let tetris_count = cells.iter()
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

    true
}
