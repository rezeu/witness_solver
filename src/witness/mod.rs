pub mod graph;
pub mod state;
pub mod region;
pub mod rules;
pub mod pruners;
pub mod debug_draw;

use std::time::Instant;

use crate::solver::{DfsStats, Pruner, PrunerChain, Satisfier, run_parallel_dfs, run_dfs};
use graph::WitnessGraph;
use state::WitnessState;
use rules::WitnessValidator;
use pruners::{ReachabilityPruner, DotReachabilityPruner, TrianglePruner, ClosedRegionPruner, has_color_constraints};
use graph::CellConstraint;

pub fn solve_file(path: &str, parallel: bool, profile: bool) -> Result<(), Box<dyn std::error::Error>> {
    let graph = WitnessGraph::from_file(path)?;
    let initial = WitnessState::new(&graph);

    println!("Puzzle: {}x{}", graph.width, graph.height);
    graph.draw_with_state(None);

    // Build pruner chain
    let pruners = build_pruner_chain(&graph);

    let satisfiers = WitnessValidator::new(&graph);

    if profile {
        let report = run_profile_bench(&graph, initial, &pruners, &satisfiers);
        report.display(&graph);
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

pub fn build_pruner_chain(graph: &WitnessGraph) -> PrunerChain<WitnessState> {
    let mut pruners = if graph.dot_nodes.is_empty() && graph.dot_edges.is_empty() {
        PrunerChain::new().add(Box::new(ReachabilityPruner))
    } else {
        PrunerChain::new().add(Box::new(DotReachabilityPruner))
    };
    if !graph.triangle_cells.is_empty() {
        pruners = pruners.add(Box::new(TrianglePruner));
    }
    let has_eliminations = graph.cells.iter().any(|c| matches!(c, CellConstraint::Elimination));
    if has_color_constraints(&graph) && !has_eliminations {
        pruners = pruners.add(Box::new(ClosedRegionPruner));
    }
    pruners
}

pub fn solve(
    graph: &WitnessGraph,
    initial: WitnessState,
    pruners: &impl Pruner<WitnessState>,
    satisfiers: &impl Satisfier<WitnessState>,
    parallel: bool,
) -> (Option<WitnessState>, DfsStats) {
    if parallel {
        run_parallel_dfs(graph, initial, pruners, satisfiers, 3)
    } else {
        run_dfs(graph, initial, pruners, satisfiers)
    }
}

/// Result of a single parallel DFS benchmark run.
pub struct ParallelResult {
    pub split_depth: usize,
    pub elapsed: std::time::Duration,
    pub nodes: u64,
    pub solution: Option<WitnessState>,
}

/// Full benchmark report from run_profile_bench().
pub struct ProfileReport {
    pub num_cpus: usize,
    pub seq_elapsed: std::time::Duration,
    pub seq_nodes: u64,
    pub seq_solution: Option<WitnessState>,
    pub parallel_results: Vec<ParallelResult>,
}

/// Run a full benchmark: sequential + parallel at depths [2,3,4,5].
/// Pure computation — no I/O, no printing.
pub fn run_profile_bench<P, Q>(
    graph: &WitnessGraph,
    initial: WitnessState,
    pruners: &P,
    satisfiers: &Q,
) -> ProfileReport
where
    P: crate::solver::Pruner<WitnessState> + Sync,
    Q: crate::solver::Satisfier<WitnessState> + Sync,
{
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    // Sequential run
    let seq_start = Instant::now();
    let (seq_sol, seq_stats) = run_dfs(graph, initial.clone(), pruners, satisfiers);
    let seq_elapsed = seq_start.elapsed();

    // Parallel runs at different split depths
    let split_depths = [2, 3, 4, 5];
    let mut parallel_results = Vec::with_capacity(split_depths.len());
    for &depth in &split_depths {
        let par_start = Instant::now();
        let (par_sol, par_stats) = run_parallel_dfs(graph, initial.clone(), pruners, satisfiers, depth);
        parallel_results.push(ParallelResult {
            split_depth: depth,
            elapsed: par_start.elapsed(),
            nodes: par_stats.node_count(),
            solution: par_sol,
        });
    }

    ProfileReport {
        num_cpus,
        seq_elapsed,
        seq_nodes: seq_stats.node_count(),
        seq_solution: seq_sol,
        parallel_results,
    }
}

impl ProfileReport {
    /// Print the full benchmark report to stdout.
    /// All I/O lives here — this is the only function that prints.
    pub fn display(&self, graph: &WitnessGraph) {
        println!("\n=== PROFILE MODE ({} logical CPUs) ===\n", self.num_cpus);

        println!("[1/3] Sequential DFS...");
        let seq_nps = self.seq_nodes as f64 / self.seq_elapsed.as_secs_f64();
        println!(
            "  nodes: {}  time: {:.3}s  throughput: {:.0} nodes/s",
            self.seq_nodes,
            self.seq_elapsed.as_secs_f64(),
            seq_nps
        );

        let mut best_time = f64::MAX;
        let mut best_depth = 0usize;
        for r in &self.parallel_results {
            println!("\n[2/3] Parallel DFS (split_depth={})...", r.split_depth);
            let par_nps = r.nodes as f64 / r.elapsed.as_secs_f64();
            let wall_speedup = self.seq_elapsed.as_secs_f64() / r.elapsed.as_secs_f64();
            let efficiency = wall_speedup / self.num_cpus as f64 * 100.0;

            println!(
                "  nodes: {}  time: {:.3}s  throughput: {:.0} nodes/s",
                r.nodes,
                r.elapsed.as_secs_f64(),
                par_nps
            );
            println!(
                "  speedup: {:.2}x  efficiency: {:.1}%  (vs {} cores)",
                wall_speedup, efficiency, self.num_cpus
            );

            if r.elapsed.as_secs_f64() < best_time {
                best_time = r.elapsed.as_secs_f64();
                best_depth = r.split_depth;
            }

            if r.solution.is_none() && self.seq_solution.is_some() {
                println!("  WARNING: parallel missed solution found by sequential!");
            }
        }

        println!("\n[3/3] Summary");
        println!(
            "  sequential:     {:.3}s  ({} nodes)",
            self.seq_elapsed.as_secs_f64(),
            self.seq_nodes
        );
        println!(
            "  best parallel:  {:.3}s  (split_depth={})",
            best_time, best_depth
        );
        println!(
            "  best speedup:   {:.2}x on {} cores ({:.1}% efficiency)",
            self.seq_elapsed.as_secs_f64() / best_time,
            self.num_cpus,
            self.seq_elapsed.as_secs_f64() / best_time / self.num_cpus as f64 * 100.0
        );

        if let Some(s) = &self.seq_solution {
            println!("\nSolution:");
            graph.draw_with_state(Some(s));
        } else {
            println!("\nNo solution found.");
        }
    }
}
