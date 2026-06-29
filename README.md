# witness_solver

High-performance Rust solver for grid puzzles inspired by *The Witness*.
The project combines a generic DFS engine with Witness-specific validation,
pruning, symmetry handling, profiling, and an egui puzzle editor.

## Run

```bash
cargo run --release -- puzzles/basic_4x4.json
cargo run --release -- puzzles/basic_4x4.json --seq
cargo run --release -- puzzles/basic_4x4.json --pruners dots --auto-split
cargo run --release -- puzzles/basic_4x4.json --profile
cargo run --release -- puzzles/basic_4x4.json --profile-json /tmp/profile.json --profile-csv /tmp/profile.csv
cargo run --release -- --gui
```

`cargo run --release` without arguments opens the GUI.

## Tests

```bash
cargo test --release
cargo test --release -- --ignored
```

Release mode is intentional: debug builds are too slow for the DFS-heavy
integration tests.

## CLI Options

- `--seq`: use sequential DFS instead of Rayon parallel DFS.
- `--split-depth <n>`: expand the first `n` levels before parallel workers run.
- `--auto-split`: choose split depth from CPU count and early branching.
- `--pruners <profile>`: `none`, `reachability`, `dots`, `triangles`, `regions`, `symmetry`, or `all`.
- `--profile`: run sequential and parallel benchmark passes.
- `--profile-json <path>` / `--profile-csv <path>`: export benchmark data.
- `--gui`: open the editor/visual solver.

## Architecture

- `src/solver/`: generic DFS, undo stack, pruner and satisfier traits.
- `src/witness/schema.rs`: puzzle JSON schema.
- `src/witness/constraints.rs`: cell constraint types.
- `src/witness/indexing.rs`: node and edge indexing formulas.
- `src/witness/graph.rs`: puzzle loading, validation, immutable precomputed topology.
- `src/witness/state.rs`: path state, move generation, apply/undo logic, symmetry edge application.
- `src/witness/rules.rs`: final constraint validator.
- `src/witness/pruners.rs`: reachability, dot, triangle, closed-region, and symmetry pruning.
- `src/gui.rs`: interactive puzzle editor, solution display, solve statistics, and path animation.

The library-level entry points are:

- `load_puzzle`
- `solve_puzzle`
- `profile_puzzle`
- `SolverConfig`
- `SolverReport`

## Supported Rules

The solver supports starts/ends, broken edges, black node and edge dots,
colored node and edge dots, squares, stars, suns, triangles, positive and
negative tetris/polyomino constraints, elimination marks, and mirror symmetry
(`x`, `y`, `xy`).

## Report Notes

The `docs/` directory is organized as a project-report source:

- [Algorithm](docs/algorithm.md)
- [Puzzle JSON Format](docs/puzzle-format.md)
- [Experiments](docs/experiments.md)
- [Screenshots](docs/screenshots/README.md)
