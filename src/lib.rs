mod dfs;
mod pruner;
mod satisfier;
mod state;
mod undo;
mod problem;
mod debug_draw;

use problem::{graph::Graph, witness_state::WitnessState as wState, witness_pruner::WitnessPruner as wPruner, witness_satisfier::WitnessSatisfier as wSatisfier};
use dfs::run_dfs;
use std::sync::Arc;

pub fn run() -> Result<(), &'static str> {
    // let graph = Graph::from_file("puzzle.txt");
    let graph = Arc::new(Graph::from_file("puzzles/puzzle.txt")?);

    let initial_state = wState::new(&graph);

    // // ---------- 构建规则 ----------
    let  pruners = wPruner::build(graph.clone());
    let  satisfiers = wSatisfier::build(graph.clone());


    // // ---------- DFS ----------

    run_dfs(&graph, initial_state, pruners, satisfiers);

    // stats.print();
    Ok(())
}
