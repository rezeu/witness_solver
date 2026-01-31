fn main() {
    if let Err(e) = witness_solver::run() {
        eprintln!("Application error: {}", e);
    }
}
