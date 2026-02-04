use crate::{
    problem::graph::Graph, pruner::Pruner, satisfier::Satisfier, state::SearchState,
    undo::UndoStack,
};

pub struct DfsStats {
    pub nodes: u64,
}

pub fn dfs<S, P, Q>(
    g: &Graph,
    state: &mut S,
    pruners: &P,
    satisfiers: &Q,
    undo: &mut UndoStack<S>,
    stats: &mut DfsStats,
    verbose: bool,
) -> bool
where
    S: SearchState + Clone,
    P: Pruner<S>,
    Q: Satisfier<S>,
{
    stats.nodes += 1;
    let node = stats.nodes.clone();

    if pruners.should_prune(state) {
        return false;
    }

    if satisfiers.is_satisfied(state) {
        println!("Solved!");
        return true;
    }

    let mut moves = Vec::new();
    state.gen_moves(g, &mut moves);

    let s = moves
        .iter()
        .map(|m| format!("{}", m))
        .collect::<Vec<String>>()
        .join(", ");
    for m in moves {
        if verbose {
            println!(
                "At node {}, choose move {} from moves {}",
                node,
                m,s
            );
            state.draw(g);
            // undo.draw();
        }
        state.apply_move(g, m, undo);

        if dfs(g, state, pruners, satisfiers, undo, stats, verbose) {
            return true;
        }

        undo.rollback(state);
    }

    false
}

pub fn run_dfs<S, P, Q>(
    g: &Graph,
    initial_state: S,
    pruners: P,
    satisfiers: Q,
    verbose: bool,
) -> DfsStats
where
    S: SearchState + Clone,
    P: Pruner<S>,
    Q: Satisfier<S>,
{
    let mut state = initial_state.clone();
    let mut undo = UndoStack::new();
    let mut stats = DfsStats { nodes: 0 };

    dfs(
        g,
        &mut state,
        &pruners,
        &satisfiers,
        &mut undo,
        &mut stats,
        verbose,
    );

    stats
}
