use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    let file = args.get(1).map(|s| s.as_str()).unwrap_or("puzzles/basic_4x4.json");

    let parallel = !args.iter().any(|a| a == "--seq");

    if let Err(e) = witness_solver::witness::solve_file(file, parallel) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
