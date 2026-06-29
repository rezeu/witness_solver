use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use rayon::prelude::*;

use crate::solver::{Pruner, Satisfier, SearchState, UndoStack};

pub struct DfsStats {
    pub nodes: AtomicU64,
    pub pruned: AtomicU64,
    pub work_items: AtomicU64,
}

impl DfsStats {
    pub fn new() -> Self {
        DfsStats {
            nodes: AtomicU64::new(0),
            pruned: AtomicU64::new(0),
            work_items: AtomicU64::new(0),
        }
    }
    pub fn node_count(&self) -> u64 {
        self.nodes.load(Ordering::Relaxed)
    }
    pub fn pruned_count(&self) -> u64 {
        self.pruned.load(Ordering::Relaxed)
    }
    pub fn work_item_count(&self) -> u64 {
        self.work_items.load(Ordering::Relaxed)
    }
}

impl Default for DfsStats {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Sequential DFS (used both standalone and inside parallel workers)
// ---------------------------------------------------------------------------

struct DfsInnerCtx<'a, S, P, Q>
where
    S: SearchState,
{
    ctx: &'a S::Ctx,
    pruners: &'a P,
    satisfiers: &'a Q,
    stats: &'a DfsStats,
    found: &'a AtomicBool,
    moves_buf: &'a mut Vec<S::Move>,
}

fn dfs_inner<S, P, Q>(
    dfs_ctx: &mut DfsInnerCtx<'_, S, P, Q>,
    state: &mut S,
    undo: &mut UndoStack<S>,
) -> Option<S>
where
    S: SearchState,
    P: Pruner<S>,
    Q: Satisfier<S>,
{
    dfs_ctx.stats.nodes.fetch_add(1, Ordering::Relaxed);

    if dfs_ctx.found.load(Ordering::Acquire) {
        return None;
    }
    if dfs_ctx.pruners.should_prune(state, dfs_ctx.ctx) {
        dfs_ctx.stats.pruned.fetch_add(1, Ordering::Relaxed);
        return None;
    }
    if dfs_ctx.satisfiers.is_satisfied(state, dfs_ctx.ctx) {
        return Some(state.clone());
    }

    let moves_start = dfs_ctx.moves_buf.len();
    state.gen_moves(dfs_ctx.ctx, dfs_ctx.moves_buf);
    let moves_end = dfs_ctx.moves_buf.len();

    for i in moves_start..moves_end {
        let mv = dfs_ctx.moves_buf[i];
        state.apply_move(dfs_ctx.ctx, mv, undo);

        if let Some(solution) = dfs_inner(dfs_ctx, state, undo) {
            return Some(solution);
        }

        undo.rollback(state);
    }

    dfs_ctx.moves_buf.truncate(moves_start);

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
    stats.work_items.store(1, Ordering::Relaxed);
    let mut state = initial;
    let mut undo = UndoStack::new();
    let mut moves_buf = Vec::new();

    let mut dfs_ctx = DfsInnerCtx {
        ctx,
        pruners,
        satisfiers,
        stats: &stats,
        found: &found,
        moves_buf: &mut moves_buf,
    };
    let solution = dfs_inner(&mut dfs_ctx, &mut state, &mut undo);
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
                stats.pruned.fetch_add(1, Ordering::Relaxed);
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

    stats.work_items.store(work.len() as u64, Ordering::Relaxed);

    let solution = work.into_par_iter().find_map_any(|state| {
        if found.load(Ordering::Acquire) {
            return None;
        }

        let mut state = state;
        let mut undo = UndoStack::new();
        let mut moves_buf = Vec::new();

        let mut dfs_ctx = DfsInnerCtx {
            ctx,
            pruners,
            satisfiers,
            stats: &stats,
            found: &found,
            moves_buf: &mut moves_buf,
        };
        let result = dfs_inner(&mut dfs_ctx, &mut state, &mut undo);
        if result.is_some() {
            found.store(true, Ordering::Release);
        }
        result
    });

    (solution, stats)
}
