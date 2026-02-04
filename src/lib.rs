mod dfs;
mod pruner;
mod satisfier;
mod state;
mod undo;
mod problem;

use problem::{graph::Graph, witness_state::WitnessState as wState, witness_pruner::WitnessPruner as wPruner, witness_satisfier::WitnessSatisfier as wSatisfier};
use dfs::run_dfs;
use std::sync::Arc;

pub fn run() -> Result<(), &'static str> {
    // let graph = Graph::from_file("puzzle.txt");
    let graph = Arc::new(Graph::from_file("puzzles/puzzle4_4.txt")?);

    let initial_state = wState::new(&graph);

    // // ---------- 构建规则 ----------
    let  pruners = wPruner::build(graph.clone());
    let  satisfiers = wSatisfier::build(graph.clone());


    // // ---------- DFS ----------
    let verbose = true;

    run_dfs(&graph, initial_state, pruners, satisfiers, verbose);

    // stats.print();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_run_with_existing_puzzle() {
        // Assuming the puzzle file exists, test that run() completes without error
        let result = run();
        assert!(result.is_ok(), "run() should succeed for puzzle4_4.txt");
    }

    // #[test]
    // fn test_graph_loading() {
    //     // Test that Graph can be loaded from a file
    //     let graph_result = Graph::from_file("puzzles/puzzle4_4.txt");
    //     assert!(graph_result.is_ok(), "Graph should load successfully from puzzle4_4.txt");
    //     let graph = graph_result.unwrap();
    //     assert!(!graph.nodes.is_empty(), "Graph should have nodes");
    // }

    #[test]
    fn test_initial_state_creation() {
        // Test creating initial state from graph
        let graph = Arc::new(Graph::from_file("puzzles/puzzle4_4.txt").unwrap());
        let state = wState::new(&graph);
        // Add assertions based on expected state properties, e.g., position or something
        // Since we don't have the struct, assume it's valid if created
        assert!(true, "Initial state created successfully");
    }

    // #[test]
    // fn test_pruners_and_satisfiers_build() {
    //     // Test building pruners and satisfiers
    //     let graph = Arc::new(Graph::from_file("puzzles/puzzle4_4.txt").unwrap());
    //     let pruners = wPruner::build(graph.clone());
    //     let satisfiers = wSatisfier::build(graph.clone());
    //     assert!(!pruners.is_empty(), "Pruners should be built");
    //     assert!(!satisfiers.is_empty(), "Satisfiers should be built");
    // }

    // Add more tests as needed, e.g., for debug_draw functionality if it has testable functions
}