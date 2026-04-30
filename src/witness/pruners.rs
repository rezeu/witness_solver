use crate::solver::Pruner;
use crate::witness::graph::{CellConstraint, WitnessGraph};
use crate::witness::region::compute_regions;
use crate::witness::rules::{check_squares_in_region, check_stars_in_region};
use crate::witness::state::WitnessState;

/// Prune if the head can no longer reach the end node through unvisited nodes
/// and unused/non-broken edges.  Single BFS per call — O(V+E) on the grid,
/// which is tiny for Witness-sized grids (≤ 15×15 ≈ 256 nodes).
pub struct ReachabilityPruner;

impl Pruner<WitnessState> for ReachabilityPruner {
    fn should_prune(&self, s: &WitnessState, g: &WitnessGraph) -> bool {
        !can_reach(s, g, s.head, g.end)
    }
}

fn can_reach(s: &WitnessState, g: &WitnessGraph, from: usize, to: usize) -> bool {
    if from == to {
        return true;
    }

    let mut visited = [0u64; 4];
    let mut stack_buf = [0usize; 289];
    bit_set(&mut visited, from);
    stack_buf[0] = from;
    let mut sp: usize = 1;

    while sp > 0 {
        sp -= 1;
        let u = stack_buf[sp];
        let (neighbors, count) = &g.adj[u];

        for i in 0..*count as usize {
            let v = neighbors[i];
            if bit_test(&visited, v) {
                continue;
            }
            if s.degrees[v] > 0 && v != to {
                continue;
            }
            let ei = g.edge_endpoints_to_idx(u, v);
            if s.used(ei) || g.is_broken(ei) {
                continue;
            }
            bit_set(&mut visited, v);
            if v == to {
                return true;
            }
            stack_buf[sp] = v;
            sp += 1;
        }
    }

    false
}

/// Extended pruner: also checks that all unvisited dot nodes are still
/// reachable from the head.
pub struct DotReachabilityPruner;

impl Pruner<WitnessState> for DotReachabilityPruner {
    fn should_prune(&self, s: &WitnessState, g: &WitnessGraph) -> bool {
        let reachable = compute_reachable(s, g);

        // End must be reachable
        if !bit_test(&reachable, g.end) {
            return true;
        }

        // All unvisited dot nodes must be reachable
        for &ni in &g.dot_nodes {
            if s.degrees[ni] == 0 && !bit_test(&reachable, ni) {
                return true;
            }
        }

        false
    }
}

fn compute_reachable(s: &WitnessState, g: &WitnessGraph) -> [u64; 4] {
    let mut reachable = [0u64; 4];
    let mut stack_buf = [0usize; 289];
    bit_set(&mut reachable, s.head);
    stack_buf[0] = s.head;
    let mut sp: usize = 1;

    while sp > 0 {
        sp -= 1;
        let u = stack_buf[sp];
        let (neighbors, count) = &g.adj[u];

        for i in 0..*count as usize {
            let v = neighbors[i];
            if bit_test(&reachable, v) {
                continue;
            }
            if s.degrees[v] > 0 && v != g.end {
                continue;
            }
            let ei = g.edge_endpoints_to_idx(u, v);
            if s.used(ei) || g.is_broken(ei) {
                continue;
            }
            bit_set(&mut reachable, v);
            stack_buf[sp] = v;
            sp += 1;
        }
    }

    reachable
}

/// Prune early if any triangle cell's constraint is already impossible.
/// For each triangle cell requiring N boundary edges:
///   - If already-used edges > N → prune (too many)
///   - If used + remaining-available < N → prune (can't reach target)
pub struct TrianglePruner;

impl Pruner<WitnessState> for TrianglePruner {
    fn should_prune(&self, s: &WitnessState, g: &WitnessGraph) -> bool {
        for &(cx, cy, count) in &g.triangle_cells {
            let required = count as usize;
            let mut used = 0usize;
            let mut available = 0usize;

            // Check 4 boundary edges of cell (cx, cy)
            // top: h_edge(cx, cy)
            let ei = g.h_edge_index(cx, cy);
            if s.used(ei) {
                used += 1;
            } else if !g.is_broken(ei) {
                available += 1;
            }
            // bottom: h_edge(cx, cy+1)
            let ei = g.h_edge_index(cx, cy + 1);
            if s.used(ei) {
                used += 1;
            } else if !g.is_broken(ei) {
                available += 1;
            }
            // left: v_edge(cx, cy)
            let ei = g.v_edge_index(cx, cy);
            if s.used(ei) {
                used += 1;
            } else if !g.is_broken(ei) {
                available += 1;
            }
            // right: v_edge(cx+1, cy)
            let ei = g.v_edge_index(cx + 1, cy);
            if s.used(ei) {
                used += 1;
            } else if !g.is_broken(ei) {
                available += 1;
            }

            if used > required || used + available < required {
                return true;
            }
        }
        false
    }
}

/// Prune by detecting cell-regions that are already "closed" — meaning no
/// future move can split them — and validating square/star color rules on
/// them mid-search instead of waiting for the full path.
///
/// A region R is closed iff every internal edge of R has both grid-node
/// endpoints unreachable from `state.head`. Sound because a future split
/// requires the path to traverse some internal edge, which requires reaching
/// at least one of its endpoints.
pub struct ClosedRegionPruner;

impl Pruner<WitnessState> for ClosedRegionPruner {
    fn should_prune(&self, s: &WitnessState, g: &WitnessGraph) -> bool {
        let reachable = compute_reachable(s, g);
        let regions = compute_regions(s, g);

        // Mark which regions are closed. Iterate every cell; for any internal
        // edge (between two same-region cells), if either endpoint is
        // reachable the region is open. We only need to check Right and Down
        // edges to cover all internal edges exactly once.
        let mut closed = [true; 256];
        let w = g.width;
        let h = g.height;
        for cy in 0..h {
            for cx in 0..w {
                let r = regions.cell_region(cx, cy) as usize;
                if !closed[r] {
                    continue;
                }
                // Right neighbor — internal edge is v_edge(cx+1, cy)
                if cx + 1 < w && regions.cell_region(cx + 1, cy) as usize == r {
                    let n1 = g.node_xy_to_idx(cx + 1, cy);
                    let n2 = g.node_xy_to_idx(cx + 1, cy + 1);
                    if bit_test(&reachable, n1) || bit_test(&reachable, n2) {
                        closed[r] = false;
                        continue;
                    }
                }
                // Down neighbor — internal edge is h_edge(cx, cy+1)
                if cy + 1 < h && regions.cell_region(cx, cy + 1) as usize == r {
                    let n1 = g.node_xy_to_idx(cx, cy + 1);
                    let n2 = g.node_xy_to_idx(cx + 1, cy + 1);
                    if bit_test(&reachable, n1) || bit_test(&reachable, n2) {
                        closed[r] = false;
                    }
                }
            }
        }

        for r in 0..regions.count {
            if !closed[r as usize] {
                continue;
            }
            if !check_squares_in_region(g, &regions, r) {
                return true;
            }
            if !check_stars_in_region(g, &regions, r) {
                return true;
            }
        }
        false
    }
}

/// Dual-source BFS: computes the set of nodes reachable from both the player
/// head and its mirror (if the puzzle has symmetry and the head is off-axis).
/// Used by both SymmetryReachabilityPruner and SymmetryDotPruner.
fn dual_compute_reachable(s: &WitnessState, g: &WitnessGraph) -> [u64; 4] {
    let mirror_end = g.symmetric_node(g.end);

    let mut reachable = [0u64; 4];
    let mut stack_buf = [0usize; 289];

    // Source 1: player head
    bit_set(&mut reachable, s.head);
    stack_buf[0] = s.head;
    let mut sp: usize = 1;

    // Source 2: mirror head (if off-axis and distinct)
    if let Some(mh) = g.symmetric_node(s.head) && mh != s.head && !bit_test(&reachable, mh) {
        bit_set(&mut reachable, mh);
        stack_buf[sp] = mh;
        sp += 1;
    }

    while sp > 0 {
        sp -= 1;
        let u = stack_buf[sp];
        let (neighbors, count) = &g.adj[u];

        for &v in neighbors.iter().take(*count as usize) {
            if bit_test(&reachable, v) {
                continue;
            }
            // End nodes can have degree > 0 (paths can terminate there)
            let is_end = v == g.end || mirror_end == Some(v);
            if s.degrees[v] > 0 && !is_end {
                continue;
            }
            let ei = g.edge_endpoints_to_idx(u, v);
            if s.used(ei) || g.is_broken(ei) {
                continue;
            }
            bit_set(&mut reachable, v);
            stack_buf[sp] = v;
            sp += 1;
        }
    }

    reachable
}

/// Prune if either the player end or the mirror end is unreachable from
/// the dual heads (player + mirror) through unvisited/unused/unbroken edges.
/// Only active for symmetry puzzles.
pub struct SymmetryReachabilityPruner;

impl Pruner<WitnessState> for SymmetryReachabilityPruner {
    fn should_prune(&self, s: &WitnessState, g: &WitnessGraph) -> bool {
        let reachable = dual_compute_reachable(s, g);

        // Player end must be reachable
        if !bit_test(&reachable, g.end) {
            return true;
        }

        // Mirror end must be reachable (if not on-axis)
        if let Some(me) = g.symmetric_node(g.end) && !bit_test(&reachable, me) {
            return true;
        }

        false
    }
}

/// Prune if any unvisited black dot node is unreachable from EITHER the
/// player path OR the mirror path in a symmetry puzzle.
/// Does NOT handle colored dots (blue/yellow) — that is deferred to P3.1.
pub struct SymmetryDotPruner;

impl Pruner<WitnessState> for SymmetryDotPruner {
    fn should_prune(&self, s: &WitnessState, g: &WitnessGraph) -> bool {
        let reachable = dual_compute_reachable(s, g);

        // Both ends must be reachable
        if !bit_test(&reachable, g.end) {
            return true;
        }
        if let Some(me) = g.symmetric_node(g.end) && !bit_test(&reachable, me) {
            return true;
        }

        // All unvisited dot nodes must be reachable via dual-source BFS
        // TODO: colored dot support (P3.1)
        for &ni in &g.dot_nodes {
            if s.degrees[ni] == 0 && !bit_test(&reachable, ni) {
                return true;
            }
        }

        false
    }
}

/// True iff this graph has any square or star constraint.
pub fn has_color_constraints(g: &WitnessGraph) -> bool {
    g.cells.iter().any(|c| matches!(c,
        CellConstraint::Square { .. } | CellConstraint::Star { .. }
    ))
}

// --- tiny bitset on [u64; 4] (256 bits, stack-allocated) ------------------

#[inline(always)]
fn bit_test(bits: &[u64; 4], i: usize) -> bool {
    let w = i >> 6;
    let b = i & 63;
    (bits[w] >> b) & 1 != 0
}

#[inline(always)]
fn bit_set(bits: &mut [u64; 4], i: usize) {
    let w = i >> 6;
    let b = i & 63;
    bits[w] |= 1u64 << b;
}
