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
use pruners::{ReachabilityPruner, DotReachabilityPruner, TrianglePruner};

pub fn solve_file(path: &str, parallel: bool, profile: bool) -> Result<(), Box<dyn std::error::Error>> {
    let graph = WitnessGraph::from_file(path)?;
    let initial = WitnessState::new(&graph);

    println!("Puzzle: {}x{}", graph.width, graph.height);
    graph.draw_with_state(None);

    // Build pruner chain
    let mut pruners = if graph.dot_nodes.is_empty() && graph.dot_edges.is_empty() {
        PrunerChain::new().add(Box::new(ReachabilityPruner))
    } else {
        PrunerChain::new().add(Box::new(DotReachabilityPruner))
    };
    if !graph.triangle_cells.is_empty() {
        pruners = pruners.add(Box::new(TrianglePruner));
    }

    let satisfiers = WitnessValidator::new(&graph);

    if profile {
        run_profile(&graph, initial, &pruners, &satisfiers)?;
    } else {
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
    }

    Ok(())
}

fn run_profile<P, Q>(
    graph: &WitnessGraph,
    initial: WitnessState,
    pruners: &P,
    satisfiers: &Q,
) -> Result<(), Box<dyn std::error::Error>>
where
    P: crate::solver::Pruner<WitnessState> + Sync,
    Q: crate::solver::Satisfier<WitnessState> + Sync,
{
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    println!("\n=== PROFILE MODE ({} logical CPUs) ===\n", num_cpus);

    // --- Sequential run ---
    println!("[1/3] Sequential DFS...");
    let seq_start = Instant::now();
    let (seq_sol, seq_stats) = run_dfs(graph, initial.clone(), pruners, satisfiers);
    let seq_elapsed = seq_start.elapsed();
    let seq_nodes = seq_stats.node_count();
    let seq_nps = seq_nodes as f64 / seq_elapsed.as_secs_f64();

    println!(
        "  nodes: {}  time: {:.3}s  throughput: {:.0} nodes/s",
        seq_nodes, seq_elapsed.as_secs_f64(), seq_nps
    );

    // --- Parallel runs with different split depths ---
    let split_depths = [2, 3, 4, 5];
    let mut best_time = f64::MAX;
    let mut best_depth = 0usize;

    for &depth in &split_depths {
        println!("\n[2/3] Parallel DFS (split_depth={})...", depth);
        let par_start = Instant::now();
        let (par_sol, par_stats) = run_parallel_dfs(graph, initial.clone(), pruners, satisfiers, depth);
        let par_elapsed = par_start.elapsed();
        let par_nodes = par_stats.node_count();
        let par_nps = par_nodes as f64 / par_elapsed.as_secs_f64();
        let wall_speedup = seq_elapsed.as_secs_f64() / par_elapsed.as_secs_f64();
        let efficiency = wall_speedup / num_cpus as f64 * 100.0;

        println!(
            "  nodes: {}  time: {:.3}s  throughput: {:.0} nodes/s",
            par_nodes, par_elapsed.as_secs_f64(), par_nps
        );
        println!(
            "  speedup: {:.2}x  efficiency: {:.1}%  (vs {} cores)",
            wall_speedup, efficiency, num_cpus
        );

        if par_elapsed.as_secs_f64() < best_time {
            best_time = par_elapsed.as_secs_f64();
            best_depth = depth;
        }

        if par_sol.is_none() && seq_sol.is_some() {
            println!("  WARNING: parallel missed solution found by sequential!");
        }
    }

    // --- Summary ---
    println!("\n[3/3] Summary");
    println!("  sequential:     {:.3}s  ({} nodes)", seq_elapsed.as_secs_f64(), seq_nodes);
    println!("  best parallel:  {:.3}s  (split_depth={})", best_time, best_depth);
    println!(
        "  best speedup:   {:.2}x on {} cores ({:.1}% efficiency)",
        seq_elapsed.as_secs_f64() / best_time,
        num_cpus,
        seq_elapsed.as_secs_f64() / best_time / num_cpus as f64 * 100.0
    );

    if let Some(s) = seq_sol {
        println!("\nSolution:");
        graph.draw_with_state(Some(&s));
    } else {
        println!("\nNo solution found.");
    }

    Ok(())
}
