use crate::solver::Satisfier;
use crate::witness::graph::{CellConstraint, WitnessGraph};
use crate::witness::region::{compute_regions, RegionMap};
use crate::witness::state::WitnessState;

/// Single satisfier that checks ALL witness rules in optimal order.
/// Region computation is deferred until needed and shared across rule checks.
pub struct WitnessValidator {
    has_region_rules: bool,
}

impl WitnessValidator {
    pub fn new(g: &WitnessGraph) -> Self {
        WitnessValidator {
            has_region_rules: g.has_region_rules,
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

        // 4. Triangle rule (no regions needed)
        if !check_triangles(s, g) {
            return false;
        }

        // 5. Region-based rules (squares, stars, tetris, elimination)
        if self.has_region_rules {
            let regions = compute_regions(s, g);
            if !check_squares(g, &regions) {
                return false;
            }
            if !check_stars(g, &regions) {
                return false;
            }
            // TODO: check_tetris, check_elimination
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
    // Every dot node must have been visited (degree > 0)
    for &ni in &g.dot_nodes {
        if s.degrees[ni] == 0 {
            return false;
        }
    }
    // Every dot edge must be used
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
    // top
    if s.used(g.h_edge_index(cx, cy)) {
        n += 1;
    }
    // bottom
    if s.used(g.h_edge_index(cx, cy + 1)) {
        n += 1;
    }
    // left
    if s.used(g.v_edge_index(cx, cy)) {
        n += 1;
    }
    // right
    if s.used(g.v_edge_index(cx + 1, cy)) {
        n += 1;
    }
    n
}

/// Each region must contain squares of only one color.
fn check_squares(g: &WitnessGraph, regions: &RegionMap) -> bool {
    for r in 0..regions.count {
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
    }
    true
}

/// For each color with at least one star in a region, the total count of
/// elements of that color (stars + squares) must be exactly 2.
fn check_stars(g: &WitnessGraph, regions: &RegionMap) -> bool {
    for r in 0..regions.count {
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
    }
    true
}
