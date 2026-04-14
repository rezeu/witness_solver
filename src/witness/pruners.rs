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

    // Stack-allocated visited bitset (supports up to 256 nodes = 16×16 grid).
    let mut visited = [0u64; 4];
    let mut stack = Vec::with_capacity(32);

    bit_set(&mut visited, from);
    stack.push(from);

    while let Some(u) = stack.pop() {
        g.for_each_neighbor(u, |v| {
            if v == to {
                // Found — we'll detect via flag after the closure.
            }
            if bit_test(&visited, v) {
                return;
            }
            // Can only pass through unvisited nodes (degree 0) or the end.
            if s.degrees[v] > 0 && v != to {
                return;
            }
            let ei = g.edge_endpoints_to_idx(u, v);
            if s.used(ei) || g.is_broken(ei) {
                return;
            }
            bit_set(&mut visited, v);
            stack.push(v);
        });

        // Check if we reached `to` as a direct neighbor.
        // (for_each_neighbor can't early-return, so check after.)
        if bit_test(&visited, to) {
            return true;
        }

        // Also check direct adjacency to `to` explicitly (for_each_neighbor
        // skips setting visited for `to` since it might have degree > 0,
        // but we DO want to reach it).
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
    let mut stack = Vec::with_capacity(32);

    bit_set(&mut reachable, s.head);
    stack.push(s.head);

    while let Some(u) = stack.pop() {
        g.for_each_neighbor(u, |v| {
            if bit_test(&reachable, v) {
                return;
            }
            if s.degrees[v] > 0 && v != g.end {
                return;
            }
            let ei = g.edge_endpoints_to_idx(u, v);
            if s.used(ei) || g.is_broken(ei) {
                return;
            }
            bit_set(&mut reachable, v);
            stack.push(v);
        });
    }

    reachable
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
