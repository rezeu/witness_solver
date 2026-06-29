use witness_solver::witness::graph::{
    GraphError, PuzzleJson, SquareJson, TetrisJson, WitnessGraph,
};

fn base_json() -> PuzzleJson {
    PuzzleJson {
        width: 2,
        height: 2,
        starts: vec![[0, 0]],
        ends: vec![[2, 2]],
        ..Default::default()
    }
}

fn invalid_message(json: PuzzleJson) -> String {
    match WitnessGraph::from_json(json) {
        Err(GraphError::InvalidPuzzle(message)) => message,
        Err(other) => panic!("expected InvalidPuzzle, got {other}"),
        Ok(_) => panic!("expected InvalidPuzzle, got valid graph"),
    }
}

#[test]
fn rejects_zero_dimensions() {
    let mut json = base_json();
    json.width = 0;

    let message = invalid_message(json);
    assert!(message.contains("width and height"));
}

#[test]
fn rejects_multiple_starts() {
    let mut json = base_json();
    json.starts.push([1, 0]);

    let message = invalid_message(json);
    assert!(message.contains("multiple start"));
}

#[test]
fn rejects_out_of_bounds_node_coordinate() {
    let mut json = base_json();
    json.node_dots.push([3, 1]);

    let message = invalid_message(json);
    assert!(message.contains("outside node bounds"));
}

#[test]
fn rejects_non_adjacent_edge() {
    let mut json = base_json();
    json.edge_dots.push([[0, 0], [2, 0]]);

    let message = invalid_message(json);
    assert!(message.contains("adjacent grid nodes"));
}

#[test]
fn rejects_duplicate_node_constraints() {
    let mut json = base_json();
    json.node_dots.push([1, 1]);
    json.colored_node_dots
        .push(witness_solver::witness::graph::ColoredDotJson {
            pos: [1, 1],
            color: 1,
        });

    let message = invalid_message(json);
    assert!(message.contains("duplicate node constraint"));
}

#[test]
fn rejects_duplicate_cell_constraints() {
    let mut json = base_json();
    json.squares.push(SquareJson {
        pos: [0, 0],
        color: 1,
    });
    json.eliminations.push([0, 0]);

    let message = invalid_message(json);
    assert!(message.contains("both square and elimination"));
}

#[test]
fn rejects_out_of_range_triangle_count() {
    let mut json = base_json();
    json.triangles
        .push(witness_solver::witness::graph::TriangleJson {
            pos: [0, 0],
            count: 4,
        });

    let message = invalid_message(json);
    assert!(message.contains("invalid count"));
}

#[test]
fn rejects_tetris_shape_that_cannot_fit_grid() {
    let mut json = base_json();
    json.tetris.push(TetrisJson {
        pos: [0, 0],
        shape: vec![[0, 0], [1, 0], [2, 0]],
        negative: false,
        can_rotate: true,
    });

    let message = invalid_message(json);
    assert!(message.contains("does not fit"));
}

#[test]
fn accepts_valid_minimal_schema() {
    assert!(WitnessGraph::from_json(base_json()).is_ok());
}
