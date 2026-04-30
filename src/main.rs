use clap::Parser;
use std::time::Instant;
use witness_solver::witness::build_pruner_chain;
use witness_solver::witness::graph::WitnessGraph;
use witness_solver::witness::rules::WitnessValidator;
use witness_solver::witness::state::WitnessState;
use witness_solver::witness::{run_profile_bench, solve};

#[derive(Parser)]
#[command(name = "witness-solver", version, about = "The Witness puzzle solver")]
struct Cli {
    /// Path to puzzle JSON file
    #[arg(default_value = "puzzles/basic_4x4.json")]
    file: String,

    /// Run sequential DFS (default: parallel)
    #[arg(long)]
    seq: bool,

    /// Run profile/benchmark mode
    #[arg(long)]
    profile: bool,
}

fn main() {
    let cli = Cli::parse();

    let graph = WitnessGraph::from_file(&cli.file).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    });

    let initial = WitnessState::new(&graph);
    let pruners = build_pruner_chain(&graph);
    let satisfiers = WitnessValidator::new(&graph);

    if cli.profile {
        let report = run_profile_bench(&graph, initial, &pruners, &satisfiers);
        report.display(&graph);
    } else {
        println!("Puzzle: {}x{}", graph.width, graph.height);
        graph.draw_with_state(None);

        let start = Instant::now();
        let (solution, stats) = solve(&graph, initial, &pruners, &satisfiers, !cli.seq);
        let elapsed = start.elapsed();

        match solution {
            Some(ref s) => {
                println!("Solved!");
                graph.draw_with_state(Some(s));
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
}
