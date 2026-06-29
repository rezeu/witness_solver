use witness_solver::witness::{
    PrunerProfile, SolverConfig, load_puzzle, profile_puzzle, solve_puzzle,
};

fn solved_with(path: &str, pruner_profile: PrunerProfile) -> bool {
    let graph = load_puzzle(path).expect("load puzzle");
    let (solution, report) = solve_puzzle(
        &graph,
        SolverConfig {
            parallel: true,
            split_depth: 2,
            auto_split: false,
            pruner_profile,
        },
    );
    assert_eq!(solution.is_some(), report.solved);
    report.solved
}

#[test]
fn all_pruner_profiles_solve_minimal_fixture() {
    for profile in [
        PrunerProfile::None,
        PrunerProfile::Reachability,
        PrunerProfile::Dots,
        PrunerProfile::Triangles,
        PrunerProfile::Regions,
        PrunerProfile::Symmetry,
        PrunerProfile::All,
    ] {
        assert!(
            solved_with("puzzles/minimal_1x1.json", profile),
            "profile {profile} should solve minimal fixture"
        );
    }
}

#[test]
fn focused_pruner_profiles_preserve_fixture_solvability() {
    for (path, profile) in [
        ("puzzles/colored_dots_2x2.json", PrunerProfile::Dots),
        ("puzzles/triangles_2x2.json", PrunerProfile::Triangles),
        ("puzzles/squares_3x3.json", PrunerProfile::Regions),
        ("puzzles/symmetry_x_4x4.json", PrunerProfile::Symmetry),
    ] {
        assert!(
            solved_with(path, profile),
            "profile {profile} should solve {path}"
        );
    }
}

#[test]
fn solver_report_records_config_and_stats() {
    let graph = load_puzzle("puzzles/minimal_1x1.json").expect("load puzzle");
    let (_solution, report) = solve_puzzle(
        &graph,
        SolverConfig {
            parallel: false,
            split_depth: 7,
            auto_split: false,
            pruner_profile: PrunerProfile::None,
        },
    );

    assert!(report.solved);
    assert!(!report.parallel);
    assert_eq!(report.split_depth, 7);
    assert_eq!(report.pruner_profile, PrunerProfile::None);
    assert!(report.nodes > 0);
    assert_eq!(report.work_items, 1);
}

#[test]
fn reports_include_per_pruner_hit_rows() {
    let graph = load_puzzle("puzzles/minimal_1x1.json").expect("load puzzle");
    let (_solution, solve_report) = solve_puzzle(
        &graph,
        SolverConfig {
            parallel: true,
            split_depth: 2,
            auto_split: false,
            pruner_profile: PrunerProfile::All,
        },
    );
    assert!(
        solve_report
            .pruner_hits
            .iter()
            .any(|hit| hit.name == "reachability")
    );

    let profile_report = profile_puzzle(
        &graph,
        SolverConfig {
            pruner_profile: PrunerProfile::All,
            ..SolverConfig::default()
        },
    );
    assert!(
        profile_report
            .seq_pruner_hits
            .iter()
            .any(|hit| hit.name == "reachability")
    );
    assert!(
        profile_report
            .parallel_results
            .iter()
            .all(|run| { run.pruner_hits.iter().any(|hit| hit.name == "reachability") })
    );
}
