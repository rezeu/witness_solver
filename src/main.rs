use clap::Parser;
use witness_solver::gui::run_gui;
use witness_solver::witness::graph::GraphError;
use witness_solver::witness::{
    PrunerProfile, SolverConfig, load_puzzle, profile_puzzle, solve_puzzle,
};

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

    /// Launch GUI editor instead of CLI
    #[arg(long)]
    gui: bool,

    /// Split depth for parallel DFS
    #[arg(long)]
    split_depth: Option<usize>,

    /// Pruner profile: none, reachability, dots, triangles, regions, symmetry, all
    #[arg(long, default_value = "all")]
    pruners: PrunerProfile,

    /// Choose split depth from CPU count and the first few expansion layers
    #[arg(long)]
    auto_split: bool,

    /// In profile mode, write machine-readable JSON report to this path
    #[arg(long)]
    profile_json: Option<String>,

    /// In profile mode, write CSV report to this path
    #[arg(long)]
    profile_csv: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    // If no explicit file was provided (user just ran the binary),
    // launch GUI by default for a better UX.
    let no_file_given = std::env::args().len() <= 1;

    if cli.gui || no_file_given {
        let path = if no_file_given && !cli.gui {
            None
        } else {
            Some(cli.file)
        };
        run_gui(path);
        return;
    }

    let graph = match load_puzzle(&cli.file) {
        Ok(g) => g,
        Err(e) => {
            print_graph_error(e);
            std::process::exit(1);
        }
    };

    let config = SolverConfig {
        parallel: !cli.seq,
        split_depth: cli.split_depth.unwrap_or(3),
        auto_split: cli.auto_split,
        pruner_profile: cli.pruners,
    };
    let profile_mode = cli.profile || cli.profile_json.is_some() || cli.profile_csv.is_some();

    if profile_mode {
        let report = profile_puzzle(&graph, config);
        report.display(&graph);
        if let Some(path) = &cli.profile_json
            && let Err(e) = report.write_json(path)
        {
            eprintln!("Error: failed to write profile JSON to {path}: {e}");
            std::process::exit(1);
        }
        if let Some(path) = &cli.profile_csv
            && let Err(e) = report.write_csv(path)
        {
            eprintln!("Error: failed to write profile CSV to {path}: {e}");
            std::process::exit(1);
        }
    } else {
        println!("Puzzle: {}x{}", graph.width(), graph.height());
        graph.draw_with_state(None);

        let (solution, report) = solve_puzzle(&graph, config);

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
            "Explored {} nodes ({} pruned) in {:.3}s ({:.0} nodes/s), split_depth={}, pruners={}",
            report.nodes,
            report.pruned,
            report.elapsed_secs,
            report.nodes as f64 / report.elapsed_secs,
            report.split_depth,
            report.pruner_profile
        );
    }
}

fn print_graph_error(e: GraphError) {
    match e {
        GraphError::MissingStart => eprintln!("Error: puzzle must have exactly one start node"),
        GraphError::MissingEnd => eprintln!("Error: puzzle must have exactly one end node"),
        GraphError::InvalidJson(e) => eprintln!("Error: failed to parse puzzle JSON: {e}"),
        GraphError::InvalidPuzzle(e) => eprintln!("Error: invalid puzzle: {e}"),
        GraphError::Io(e) => eprintln!("Error: failed to read puzzle file: {e}"),
    }
}
