use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;

use crate::solver::{Pruner, Satisfier, SearchState, UndoStack};

pub struct DfsStats {
    pub nodes: AtomicU64,
}

impl DfsStats {
    pub fn new() -> Self {
        DfsStats {
            nodes: AtomicU64::new(0),
        }
    }
    pub fn node_count(&self) -> u64 {
        self.nodes.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Sequential DFS (used both standalone and inside parallel workers)
// ---------------------------------------------------------------------------

fn dfs_inner<S, P, Q>(
    ctx: &S::Ctx,
    state: &mut S,
    pruners: &P,
    satisfiers: &Q,
    undo: &mut UndoStack<S>,
    stats: &DfsStats,
    found: &AtomicBool,
    moves_buf: &mut Vec<S::Move>,
) -> Option<S>
where
    S: SearchState,
    P: Pruner<S>,
    Q: Satisfier<S>,
{
    stats.nodes.fetch_add(1, Ordering::Relaxed);

    if found.load(Ordering::Relaxed) {
        return None;
    }
    if pruners.should_prune(state, ctx) {
        return None;
    }
    if satisfiers.is_satisfied(state, ctx) {
        return Some(state.clone());
    }

    let moves_start = moves_buf.len();
    state.gen_moves(ctx, moves_buf);
    let moves_end = moves_buf.len();

    for i in moves_start..moves_end {
        let mv = moves_buf[i];
        state.apply_move(ctx, mv, undo);

        if let Some(solution) = dfs_inner(ctx, state, pruners, satisfiers, undo, stats, found, moves_buf) {
            return Some(solution);
        }

        undo.rollback(state);
    }

    moves_buf.truncate(moves_start);

    None
}

// ---------------------------------------------------------------------------
// Public API: sequential
// ---------------------------------------------------------------------------

pub fn run_dfs<S, P, Q>(
    ctx: &S::Ctx,
    initial: S,
    pruners: &P,
    satisfiers: &Q,
) -> (Option<S>, DfsStats)
where
    S: SearchState,
    P: Pruner<S>,
    Q: Satisfier<S>,
{
    let found = AtomicBool::new(false);
    let stats = DfsStats::new();
    let mut state = initial;
    let mut undo = UndoStack::new();
    let mut moves_buf = Vec::new();

    let solution = dfs_inner(ctx, &mut state, pruners, satisfiers, &mut undo, &stats, &found, &mut moves_buf);
    (solution, stats)
}

// ---------------------------------------------------------------------------
// Public API: parallel (rayon work-stealing)
// ---------------------------------------------------------------------------

/// Expand the first `split_depth` levels of the search tree, then solve
/// each subtree in parallel.  `split_depth = 2` is a good default for
/// grid puzzles with branching factor ~3-4.
pub fn run_parallel_dfs<S, P, Q>(
    ctx: &S::Ctx,
    initial: S,
    pruners: &P,
    satisfiers: &Q,
    split_depth: usize,
) -> (Option<S>, DfsStats)
where
    S: SearchState + Sync,
    P: Pruner<S> + Sync,
    Q: Satisfier<S> + Sync,
{
    let found = AtomicBool::new(false);
    let stats = DfsStats::new();

    // Generate work items by expanding the first `split_depth` levels.
    let mut work: Vec<S> = vec![initial];

    for _ in 0..split_depth {
        let mut next = Vec::new();
        for state in work.drain(..) {
            stats.nodes.fetch_add(1, Ordering::Relaxed);

            if pruners.should_prune(&state, ctx) {
                continue;
            }
            if satisfiers.is_satisfied(&state, ctx) {
                return (Some(state), stats);
            }

            let mut moves = Vec::new();
            state.gen_moves(ctx, &mut moves);

            // Clone for all moves except the last (move the state into it).
            if let Some(last_mv) = moves.pop() {
                for &mv in &moves {
                    let mut s = state.clone();
                    let mut undo = UndoStack::new();
                    s.apply_move(ctx, mv, &mut undo);
                    next.push(s);
                }
                let mut s = state;
                let mut undo = UndoStack::new();
                s.apply_move(ctx, last_mv, &mut undo);
                next.push(s);
            }
        }
        work = next;
    }

    let solution = work.into_par_iter().find_map_any(|state| {
        if found.load(Ordering::Relaxed) {
            return None;
        }

        let mut state = state;
        let mut undo = UndoStack::new();
        let mut moves_buf = Vec::new();

        let result = dfs_inner(ctx, &mut state, pruners, satisfiers, &mut undo, &stats, &found, &mut moves_buf);
        if result.is_some() {
            found.store(true, Ordering::Relaxed);
        }
        result
    });

    (solution, stats)
}
