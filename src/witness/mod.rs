pub mod constraints;
pub mod debug_draw;
pub mod graph;
pub mod indexing;
pub mod pruners;
pub mod region;
pub mod rules;
pub mod schema;
pub mod state;
pub mod types;

use serde::Serialize;
use std::fmt::Write as _;
use std::time::Instant;

use crate::solver::{
    DfsStats, Pruner, PrunerChain, PrunerHitStats, Satisfier, SearchState, UndoStack, run_dfs,
    run_parallel_dfs,
};
use graph::CellConstraint;
use graph::{GraphError, WitnessGraph};
use pruners::{
    ClosedRegionPruner, DotReachabilityPruner, ReachabilityPruner, SymmetryDotPruner,
    SymmetryReachabilityPruner, TrianglePruner, has_color_constraints,
};
use state::WitnessState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrunerProfile {
    None,
    Reachability,
    Dots,
    Triangles,
    Regions,
    Symmetry,
    All,
}

impl Default for PrunerProfile {
    fn default() -> Self {
        Self::All
    }
}

impl std::fmt::Display for PrunerProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            PrunerProfile::None => "none",
            PrunerProfile::Reachability => "reachability",
            PrunerProfile::Dots => "dots",
            PrunerProfile::Triangles => "triangles",
            PrunerProfile::Regions => "regions",
            PrunerProfile::Symmetry => "symmetry",
            PrunerProfile::All => "all",
        };
        f.write_str(name)
    }
}

impl std::str::FromStr for PrunerProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "reachability" => Ok(Self::Reachability),
            "dots" => Ok(Self::Dots),
            "triangles" => Ok(Self::Triangles),
            "regions" => Ok(Self::Regions),
            "symmetry" => Ok(Self::Symmetry),
            "all" => Ok(Self::All),
            _ => Err(format!(
                "unknown pruner profile '{value}'; expected one of none, reachability, dots, triangles, regions, symmetry, all"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SolverConfig {
    pub parallel: bool,
    pub split_depth: usize,
    pub auto_split: bool,
    pub pruner_profile: PrunerProfile,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            parallel: true,
            split_depth: 3,
            auto_split: false,
            pruner_profile: PrunerProfile::All,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SolverReport {
    pub solved: bool,
    pub elapsed_secs: f64,
    pub nodes: u64,
    pub pruned: u64,
    pub pruner_hits: Vec<PrunerHitStats>,
    pub work_items: u64,
    pub parallel: bool,
    pub split_depth: usize,
    pub pruner_profile: PrunerProfile,
}

pub fn load_puzzle(path: &str) -> Result<WitnessGraph, GraphError> {
    WitnessGraph::from_file(path)
}

pub fn build_pruner_chain(graph: &WitnessGraph) -> PrunerChain<WitnessState> {
    build_pruner_chain_with_profile(graph, PrunerProfile::All)
}

pub fn build_pruner_chain_with_profile(
    graph: &WitnessGraph,
    profile: PrunerProfile,
) -> PrunerChain<WitnessState> {
    let mut pruners = PrunerChain::new();

    match profile {
        PrunerProfile::None => pruners,
        PrunerProfile::Reachability => {
            pruners.with_named("reachability", Box::new(ReachabilityPruner))
        }
        PrunerProfile::Dots => add_dot_or_reachability_pruner(pruners, graph),
        PrunerProfile::Triangles => {
            pruners = add_dot_or_reachability_pruner(pruners, graph);
            add_triangle_pruner(pruners, graph)
        }
        PrunerProfile::Regions => {
            pruners = add_dot_or_reachability_pruner(pruners, graph);
            pruners = add_triangle_pruner(pruners, graph);
            add_region_pruner(pruners, graph)
        }
        PrunerProfile::Symmetry => {
            pruners = add_dot_or_reachability_pruner(pruners, graph);
            add_symmetry_pruners(pruners, graph)
        }
        PrunerProfile::All => {
            pruners = add_dot_or_reachability_pruner(pruners, graph);
            pruners = add_triangle_pruner(pruners, graph);
            pruners = add_symmetry_pruners(pruners, graph);
            add_region_pruner(pruners, graph)
        }
    }
}

fn has_any_dots(graph: &WitnessGraph) -> bool {
    !graph.dot_nodes.is_empty()
        || !graph.dot_edges.is_empty()
        || !graph.colored_dot_nodes.is_empty()
        || !graph.colored_dot_edges.is_empty()
}

fn add_dot_or_reachability_pruner(
    pruners: PrunerChain<WitnessState>,
    graph: &WitnessGraph,
) -> PrunerChain<WitnessState> {
    if has_any_dots(graph) {
        pruners.with_named("dots", Box::new(DotReachabilityPruner))
    } else {
        pruners.with_named("reachability", Box::new(ReachabilityPruner))
    }
}

fn add_triangle_pruner(
    pruners: PrunerChain<WitnessState>,
    graph: &WitnessGraph,
) -> PrunerChain<WitnessState> {
    if graph.triangle_cells.is_empty() {
        pruners
    } else {
        pruners.with_named("triangles", Box::new(TrianglePruner))
    }
}

fn add_symmetry_pruners(
    mut pruners: PrunerChain<WitnessState>,
    graph: &WitnessGraph,
) -> PrunerChain<WitnessState> {
    if graph.symmetry.is_some() {
        pruners = pruners.with_named(
            "symmetry-reachability",
            Box::new(SymmetryReachabilityPruner),
        );
        if has_any_dots(graph) {
            pruners = pruners.with_named("symmetry-dots", Box::new(SymmetryDotPruner));
        }
    }
    pruners
}

fn add_region_pruner(
    pruners: PrunerChain<WitnessState>,
    graph: &WitnessGraph,
) -> PrunerChain<WitnessState> {
    let has_eliminations = graph
        .cells
        .iter()
        .any(|c| matches!(c, CellConstraint::Elimination));
    if has_color_constraints(graph) && !has_eliminations {
        pruners.with_named("regions", Box::new(ClosedRegionPruner))
    } else {
        pruners
    }
}

pub fn solve_puzzle(
    graph: &WitnessGraph,
    config: SolverConfig,
) -> (Option<WitnessState>, SolverReport) {
    let initial = WitnessState::new(graph);
    let pruners = build_pruner_chain_with_profile(graph, config.pruner_profile);
    let satisfiers = rules::WitnessValidator::new(graph);
    let split_depth = if config.parallel && config.auto_split {
        let split_pruners = build_pruner_chain_with_profile(graph, config.pruner_profile);
        auto_split_depth(graph, &split_pruners, &satisfiers, 5)
    } else {
        config.split_depth
    };

    let start = Instant::now();
    let (solution, stats) = solve(
        graph,
        initial,
        &pruners,
        &satisfiers,
        config.parallel,
        split_depth,
    );
    let elapsed = start.elapsed();
    let report = SolverReport {
        solved: solution.is_some(),
        elapsed_secs: elapsed.as_secs_f64(),
        nodes: stats.node_count(),
        pruned: stats.pruned_count(),
        pruner_hits: pruners.hit_stats(),
        work_items: stats.work_item_count(),
        parallel: config.parallel,
        split_depth,
        pruner_profile: config.pruner_profile,
    };

    (solution, report)
}

pub fn profile_puzzle(graph: &WitnessGraph, config: SolverConfig) -> ProfileReport {
    let initial = WitnessState::new(graph);
    let pruners = build_pruner_chain_with_profile(graph, config.pruner_profile);
    let satisfiers = rules::WitnessValidator::new(graph);
    run_profile_bench_with_profile(graph, initial, &pruners, &satisfiers, config.pruner_profile)
}

pub fn auto_split_depth<P, Q>(
    graph: &WitnessGraph,
    pruners: &P,
    satisfiers: &Q,
    max_depth: usize,
) -> usize
where
    P: Pruner<WitnessState>,
    Q: Satisfier<WitnessState>,
{
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    if cpus <= 1 {
        return 0;
    }

    let target = cpus.saturating_mul(4);
    let mut work = vec![WitnessState::new(graph)];
    let mut best_depth = 0usize;

    for depth in 0..max_depth {
        if work.len() >= target {
            return best_depth;
        }

        let mut next = Vec::new();
        for state in work.drain(..) {
            if pruners.should_prune(&state, graph) || satisfiers.is_satisfied(&state, graph) {
                continue;
            }

            let mut moves = Vec::new();
            state.gen_moves(graph, &mut moves);
            for mv in moves {
                let mut s = state.clone();
                let mut undo = UndoStack::new();
                s.apply_move(graph, mv, &mut undo);
                next.push(s);
            }
        }

        if next.is_empty() {
            return best_depth;
        }

        work = next;
        best_depth = depth + 1;
    }

    best_depth
}

pub fn solve(
    graph: &WitnessGraph,
    initial: WitnessState,
    pruners: &impl Pruner<WitnessState>,
    satisfiers: &impl Satisfier<WitnessState>,
    parallel: bool,
    split_depth: usize,
) -> (Option<WitnessState>, DfsStats) {
    if parallel {
        run_parallel_dfs(graph, initial, pruners, satisfiers, split_depth)
    } else {
        run_dfs(graph, initial, pruners, satisfiers)
    }
}

/// Result of a single parallel DFS benchmark run.
pub struct ParallelResult {
    pub split_depth: usize,
    pub elapsed: std::time::Duration,
    pub nodes: u64,
    pub pruned: u64,
    pub pruner_hits: Vec<PrunerHitStats>,
    pub work_items: u64,
    pub solution: Option<WitnessState>,
}

/// Full benchmark report from run_profile_bench().
pub struct ProfileReport {
    pub num_cpus: usize,
    pub pruner_profile: PrunerProfile,
    pub seq_elapsed: std::time::Duration,
    pub seq_nodes: u64,
    pub seq_pruned: u64,
    pub seq_pruner_hits: Vec<PrunerHitStats>,
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
    run_profile_bench_with_profile(graph, initial, pruners, satisfiers, PrunerProfile::All)
}

pub fn run_profile_bench_with_profile<P, Q>(
    graph: &WitnessGraph,
    initial: WitnessState,
    pruners: &P,
    satisfiers: &Q,
    pruner_profile: PrunerProfile,
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
    let seq_pruner_hits = pruners.hit_stats();

    // Parallel runs at different split depths
    let split_depths = [2, 3, 4, 5];
    let mut parallel_results = Vec::with_capacity(split_depths.len());
    for &depth in &split_depths {
        let before_hits = pruners.hit_stats();
        let par_start = Instant::now();
        let (par_sol, par_stats) =
            run_parallel_dfs(graph, initial.clone(), pruners, satisfiers, depth);
        let pruner_hits = diff_pruner_hits(&before_hits, &pruners.hit_stats());
        parallel_results.push(ParallelResult {
            split_depth: depth,
            elapsed: par_start.elapsed(),
            nodes: par_stats.node_count(),
            pruned: par_stats.pruned_count(),
            pruner_hits,
            work_items: par_stats.work_item_count(),
            solution: par_sol,
        });
    }

    ProfileReport {
        num_cpus,
        pruner_profile,
        seq_elapsed,
        seq_nodes: seq_stats.node_count(),
        seq_pruned: seq_stats.pruned_count(),
        seq_pruner_hits,
        seq_solution: seq_sol,
        parallel_results,
    }
}

impl ProfileReport {
    fn export(&self) -> ProfileExport {
        let best_split_depth = self
            .parallel_results
            .iter()
            .min_by_key(|r| r.elapsed)
            .map(|r| r.split_depth);

        ProfileExport {
            num_cpus: self.num_cpus,
            pruner_profile: self.pruner_profile,
            sequential: RunExport {
                mode: "sequential",
                split_depth: None,
                elapsed_secs: self.seq_elapsed.as_secs_f64(),
                nodes: self.seq_nodes,
                pruned: self.seq_pruned,
                pruner_hits: self.seq_pruner_hits.clone(),
                work_items: 1,
                solved: self.seq_solution.is_some(),
            },
            parallel: self
                .parallel_results
                .iter()
                .map(|r| RunExport {
                    mode: "parallel",
                    split_depth: Some(r.split_depth),
                    elapsed_secs: r.elapsed.as_secs_f64(),
                    nodes: r.nodes,
                    pruned: r.pruned,
                    pruner_hits: r.pruner_hits.clone(),
                    work_items: r.work_items,
                    solved: r.solution.is_some(),
                })
                .collect(),
            best_split_depth,
        }
    }

    pub fn write_json(&self, path: &str) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(&self.export()).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    pub fn write_csv(&self, path: &str) -> std::io::Result<()> {
        let mut text = String::from(
            "mode,split_depth,elapsed_secs,nodes,pruned,pruner_hits,work_items,solved,pruner_profile\n",
        );
        let export = self.export();
        let _ = writeln!(
            text,
            "{},{},{:.9},{},{},{},{},{},{}",
            export.sequential.mode,
            "",
            export.sequential.elapsed_secs,
            export.sequential.nodes,
            export.sequential.pruned,
            format_pruner_hits_csv(&export.sequential.pruner_hits),
            export.sequential.work_items,
            export.sequential.solved,
            export.pruner_profile
        );
        for r in &export.parallel {
            let _ = writeln!(
                text,
                "{},{},{:.9},{},{},{},{},{},{}",
                r.mode,
                r.split_depth.unwrap_or_default(),
                r.elapsed_secs,
                r.nodes,
                r.pruned,
                format_pruner_hits_csv(&r.pruner_hits),
                r.work_items,
                r.solved,
                export.pruner_profile
            );
        }
        std::fs::write(path, text)
    }

    /// Print the full benchmark report to stdout.
    /// All I/O lives here — this is the only function that prints.
    pub fn display(&self, graph: &WitnessGraph) {
        println!(
            "\n=== PROFILE MODE ({} logical CPUs, pruners={}) ===\n",
            self.num_cpus, self.pruner_profile
        );

        println!("[1/3] Sequential DFS...");
        let seq_nps = self.seq_nodes as f64 / self.seq_elapsed.as_secs_f64();
        println!(
            "  nodes: {}  pruned: {}  time: {:.3}s  throughput: {:.0} nodes/s",
            self.seq_nodes,
            self.seq_pruned,
            self.seq_elapsed.as_secs_f64(),
            seq_nps
        );
        print_pruner_hits(&self.seq_pruner_hits);

        let mut best_time = f64::MAX;
        let mut best_depth = 0usize;
        for r in &self.parallel_results {
            println!("\n[2/3] Parallel DFS (split_depth={})...", r.split_depth);
            let par_nps = r.nodes as f64 / r.elapsed.as_secs_f64();
            let wall_speedup = self.seq_elapsed.as_secs_f64() / r.elapsed.as_secs_f64();
            let efficiency = wall_speedup / self.num_cpus as f64 * 100.0;

            println!(
                "  nodes: {}  pruned: {}  work: {}  time: {:.3}s  throughput: {:.0} nodes/s",
                r.nodes,
                r.pruned,
                r.work_items,
                r.elapsed.as_secs_f64(),
                par_nps
            );
            print_pruner_hits(&r.pruner_hits);
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

fn diff_pruner_hits(before: &[PrunerHitStats], after: &[PrunerHitStats]) -> Vec<PrunerHitStats> {
    after
        .iter()
        .map(|after_hit| {
            let before_hits = before
                .iter()
                .find(|before_hit| before_hit.name == after_hit.name)
                .map_or(0, |before_hit| before_hit.hits);
            PrunerHitStats {
                name: after_hit.name,
                hits: after_hit.hits.saturating_sub(before_hits),
            }
        })
        .collect()
}

fn print_pruner_hits(hits: &[PrunerHitStats]) {
    if hits.is_empty() {
        return;
    }

    print!("  pruner hits:");
    for hit in hits {
        print!(" {}={}", hit.name, hit.hits);
    }
    println!();
}

fn format_pruner_hits_csv(hits: &[PrunerHitStats]) -> String {
    hits.iter()
        .map(|hit| format!("{}:{}", hit.name, hit.hits))
        .collect::<Vec<_>>()
        .join("|")
}

#[derive(Serialize)]
struct ProfileExport {
    num_cpus: usize,
    pruner_profile: PrunerProfile,
    sequential: RunExport,
    parallel: Vec<RunExport>,
    best_split_depth: Option<usize>,
}

#[derive(Serialize)]
struct RunExport {
    mode: &'static str,
    split_depth: Option<usize>,
    elapsed_secs: f64,
    nodes: u64,
    pruned: u64,
    pruner_hits: Vec<PrunerHitStats>,
    work_items: u64,
    solved: bool,
}
