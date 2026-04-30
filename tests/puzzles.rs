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

// ---------------------------------------------------------------------------
// Symmetry test — documents what needs to change in the validator.
// ---------------------------------------------------------------------------
//
// The current solver finds a path that ignores the symmetry constraint.
// This test checks that the solution respects X-axis mirror symmetry.
// It FAILS because the unmodified solver doesn't enforce symmetry.
//
// SYMMETRY VALIDATOR CHANGES NEEDED:
// - check_degrees: expects 2 nodes with degree=1, symmetry has 4
//   (player start/end + mirror start/end)
// - gen_moves: only allows revisiting ctx.end; symmetry needs revisiting
//   both ends (player end + mirror end)
// - is_satisfied: checks head==end; with symmetry, both head==end AND
//   mirror_head==mirror_end must be true
// - check_dots: all dots must have degree>0; with mirror path, some dot
//   nodes may be unreachable by the player path alone
//
// Once T14-T18 are complete, replace with:
//   puzzle_test!(symmetry_x_4x4, "puzzles/symmetry_x_4x4.json");
// and remove #[ignore].
/// Test that symmetry puzzles fail with the unmodified solver.
/// The solver finds a path that is internally consistent but does
/// not respect the symmetry constraint.
#[ignore]
#[test]
fn symmetry_x_4x4() {
    let graph = WitnessGraph::from_file("puzzles/symmetry_x_4x4.json")
        .expect("load symmetry puzzle");
    assert!(graph.symmetry.is_some(), "expected x-symmetry");

    let solution = solve("puzzles/symmetry_x_4x4.json");
    assert!(solution.is_some(), "expected a solution");
    let s = solution.unwrap();

    // Verify mirror-symmetry: every off-axis edge must have its mirror
    // edge traversed. The current solver ignores symmetry, so this
    // assertion FAILS for non-trivial paths.
    let num_slots = graph.num_edge_slots();
    for ei in 0..num_slots {
        if s.used(ei) {
            if let Some(mirror_ei) = graph.symmetric_edge(ei) {
                assert!(
                    s.used(mirror_ei),
                    "SYMMETRY VIOLATION: edge {} used but its mirror {} is not",
                    ei,
                    mirror_ei,
                );
            }
        }
    }
}
