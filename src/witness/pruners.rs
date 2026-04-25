use crate::solver::Pruner;
use crate::witness::graph::WitnessGraph;
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
