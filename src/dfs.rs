use crate::{
    problem::graph::Graph, pruner::Pruner, satisfier::Satisfier, state::SearchState, undo::UndoStack
};

pub struct DfsStats {
    pub nodes: u64 ,
}


pub fn dfs<S,P,Q>(
    g:&Graph,
    state: &mut S,
    pruners: &P,
    satisfiers: &Q,
    undo: &mut UndoStack<S>,
    stats: &mut DfsStats,
) -> bool
where
    S: SearchState+ Clone,
    P: Pruner<S>,
    Q: Satisfier<S>,
{
    stats.nodes += 1;

    if pruners.should_prune(state) {
        return false;
    }

    if satisfiers.is_satisfied(state) {
        println!("Solved!");
        return true;
    }

    let mut moves = Vec::new();
    state.gen_moves(g,&mut moves);

    for m in moves {
        state.apply_move(g, m, undo);

        if dfs(g, state, pruners,satisfiers, undo, stats) {
            return true;
        }

        undo.rollback(state);
    }

    false
}

pub fn run_dfs<S,P,Q>(
    g:&Graph,
    initial_state: S,
    pruners: P,
    satisfiers: Q,
) -> DfsStats
where
    S: SearchState + Clone,
    P: Pruner<S>,
    Q: Satisfier<S>,
{
    let mut state = initial_state.clone();
    let mut undo = UndoStack::new();
    let mut stats = DfsStats { nodes: 0 };

    dfs(g, &mut state, &pruners, &satisfiers, &mut undo, &mut stats);

    stats
}
