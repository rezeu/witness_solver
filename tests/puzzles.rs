use witness_solver::witness::build_pruner_chain;
use witness_solver::witness::graph::WitnessGraph;
use witness_solver::witness::rules::WitnessValidator;
use witness_solver::witness::state::WitnessState;

fn solve(path: &str) -> Option<WitnessState> {
    let graph = WitnessGraph::from_file(path).expect("load puzzle");
    let initial = WitnessState::new(&graph);
    let pruners = build_pruner_chain(&graph);
    let satisfiers = WitnessValidator::new(&graph);
    let (sol, _stats) = witness_solver::witness::solve(&graph, initial, &pruners, &satisfiers, true);
    sol
}

macro_rules! puzzle_test {
    ($name:ident, $file:expr) => {
        #[test]
        fn $name() {
            assert!(
                solve($file).is_some(),
                "expected a solution for {}",
                $file
            );
        }
    };
    ($name:ident, $file:expr, ignore) => {
        #[test]
        #[ignore]
        fn $name() {
            assert!(
                solve($file).is_some(),
                "expected a solution for {}",
                $file
            );
        }
    };
}

puzzle_test!(basic_4x4, "puzzles/basic_4x4.json");
puzzle_test!(dots_3x3, "puzzles/dots_3x3.json");
puzzle_test!(squares_3x3, "puzzles/squares_3x3.json");
puzzle_test!(triangles_2x2, "puzzles/triangles_2x2.json");
puzzle_test!(triangles_4x4, "puzzles/triangles_4x4.json");
puzzle_test!(tetris_2x2, "puzzles/tetris_2x2.json");
puzzle_test!(tetris_3x3, "puzzles/tetris_3x3.json");
puzzle_test!(tetris_negative_3x3, "puzzles/tetris_negative_3x3.json");
puzzle_test!(elimination_2x2, "puzzles/elimination_2x2.json");
puzzle_test!(elimination_mixed_3x3, "puzzles/elimination_mixed_3x3.json");
puzzle_test!(mixed_4x4, "puzzles/mixed_4x4.json");
puzzle_test!(everything_4x4, "puzzles/everything_4x4.json");
puzzle_test!(hard_5x5, "puzzles/hard_5x5.json");
puzzle_test!(hard_6x6, "puzzles/hard_6x6.json");
puzzle_test!(stress_mixed_6x6, "puzzles/stress_mixed_6x6.json");

// Marked #[ignore] — known to exceed 5 minutes.
// Run explicitly with `cargo test --release -- --ignored`.
puzzle_test!(stress_7x7, "puzzles/stress_7x7.json", ignore);

puzzle_test!(symmetry_x_4x4, "puzzles/symmetry_x_4x4.json");

#[test]
fn symmetry_unsolvable() {
    let result = solve("puzzles/symmetry_unsolvable.json");
    assert!(result.is_none(), "unsolvable symmetry puzzle should return None");
}
