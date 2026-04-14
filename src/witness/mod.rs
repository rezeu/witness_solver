pub mod graph;
pub mod state;
pub mod region;
pub mod rules;
pub mod pruners;
pub mod debug_draw;

use std::time::Instant;

use crate::solver::{PrunerChain, run_parallel_dfs, run_dfs};
use graph::WitnessGraph;
use state::WitnessState;
use rules::WitnessValidator;
use pruners::{ReachabilityPruner, DotReachabilityPruner};

pub fn solve_file(path: &str, parallel: bool) -> Result<(), Box<dyn std::error::Error>> {
    let graph = WitnessGraph::from_file(path)?;
    let initial = WitnessState::new(&graph);

    println!("Puzzle: {}x{}", graph.width, graph.height);
    graph.draw_with_state(None);

    // Build pruner chain
    let pruners = if graph.dot_nodes.is_empty() && graph.dot_edges.is_empty() {
        PrunerChain::new().add(Box::new(ReachabilityPruner))
    } else {
        PrunerChain::new().add(Box::new(DotReachabilityPruner))
    };

    let satisfiers = WitnessValidator::new(&graph);

    let start = Instant::now();

    let (solution, stats) = if parallel {
        run_parallel_dfs(&graph, initial, &pruners, &satisfiers, 3)
    } else {
        run_dfs(&graph, initial, &pruners, &satisfiers)
    };

    let elapsed = start.elapsed();

    match solution {
        Some(s) => {
            println!("Solved!");
            graph.draw_with_state(Some(&s));
        }
        None => {
            println!("No solution found.");
        }
    }

    println!(
        "Explored {} nodes in {:.3}s ({:.0} nodes/s)",
        stats.node_count(),
        elapsed.as_secs_f64(),
        stats.node_count() as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}
