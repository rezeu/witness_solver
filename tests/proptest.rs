use proptest::prelude::*;
use std::collections::HashSet;
use witness_solver::solver::state::SearchState;
use witness_solver::solver::undo::UndoStack;
use witness_solver::witness::graph::{PuzzleJson, WitnessGraph};
use witness_solver::witness::state::WitnessState;

fn make_4x4_graph() -> WitnessGraph {
    let json = PuzzleJson {
        width: 4,
        height: 4,
        starts: vec![[0, 0]],
        ends: vec![[4, 4]],
        symmetry: None,
        node_dots: vec![],
        edge_dots: vec![],
        broken_edges: vec![],
        squares: vec![],
        stars: vec![],
        triangles: vec![],
        tetris: vec![],
        eliminations: vec![],
        sun_cells: vec![],
        colored_node_dots: vec![],
        colored_edge_dots: vec![],
    };
    WitnessGraph::from_json(json).unwrap()
}

fn edge_idx_to_endpoints_4x4(ei: usize) -> ([usize; 2], [usize; 2]) {
    let real = ei >> 1;
    let w = 4;
    if ei & 1 == 0 {
        let y = real / w;
        let x = real % w;
        ([x, y], [x + 1, y])
    } else {
        let y = real / (w + 1);
        let x = real % (w + 1);
        ([x, y], [x, y + 1])
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn proptest_apply_undo_roundtrip(edges in prop::collection::vec(0usize..40, 1..=10)) {
        let unique: HashSet<usize> = edges.iter().copied().collect();
        prop_assume!(unique.len() == edges.len());

        let graph = make_4x4_graph();
        let mut state = WitnessState::new(&graph);
        let mut undo = UndoStack::new();

        let initial_used = state.used_edges.clone();
        let initial_degrees = state.degrees.clone();
        let initial_head = state.head;

        for &ei in &edges {
            state.apply_move(&graph, ei, &mut undo);
        }

        for _ in 0..edges.len() {
            undo.rollback(&mut state);
        }

        prop_assert_eq!(&state.used_edges, &initial_used);
        prop_assert_eq!(&state.degrees, &initial_degrees);
        prop_assert_eq!(state.head, initial_head);
    }

    #[test]
    fn proptest_gen_moves_excludes_broken(edge_idx in 0usize..40) {
        let (u_xy, v_xy) = edge_idx_to_endpoints_4x4(edge_idx);
        let json = PuzzleJson {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[4, 4]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![[u_xy, v_xy]],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            sun_cells: vec![],
            eliminations: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
        };
        let graph = WitnessGraph::from_json(json).unwrap();
        let state = WitnessState::new(&graph);
        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);
        prop_assert!(!moves.contains(&edge_idx));
    }

    #[test]
    fn proptest_gen_moves_excludes_used(edge_idx in 0usize..40) {
        let graph = make_4x4_graph();

        let initial_state = WitnessState::new(&graph);
        let mut initial_moves = Vec::new();
        initial_state.gen_moves(&graph, &mut initial_moves);
        prop_assume!(initial_moves.contains(&edge_idx));

        let mut state = WitnessState::new(&graph);
        let mut undo = UndoStack::new();
        state.apply_move(&graph, edge_idx, &mut undo);

        let mut moves = Vec::new();
        state.gen_moves(&graph, &mut moves);
        prop_assert!(!moves.contains(&edge_idx));
    }
}
