# Algorithm

## Model

A puzzle is a rectangular cell grid with `(width + 1) * (height + 1)` graph
nodes and horizontal/vertical edge slots. A search state stores:

- used edge bitset
- node degrees
- current head node

The solver accepts a solution only when the head reaches the end, endpoint
degrees match the expected path topology, all required dots are traversed, and
all region constraints validate.

Core model files:

- `schema.rs`: serde-facing puzzle JSON schema.
- `constraints.rs`: Witness cell constraint enum.
- `types.rs`: semantic aliases for node ids, edge ids, and positions.
- `indexing.rs`: pure node/edge indexing formulas.
- `graph.rs`: validated immutable graph construction and precomputed topology.

`WitnessGraph` fields are crate-visible for hot internal code paths and exposed
to library users through read-only accessor methods.

## DFS and Undo

`src/solver/dfs.rs` implements a generic DFS over the `SearchState` trait.
Witness moves are edge indices. `WitnessState::apply_move` records reversible
changes in `UndoStack`, so recursive DFS mutates in place and rolls back rather
than cloning each state.

Parallel DFS expands a configurable prefix depth into independent work items,
then uses Rayon work stealing. `--auto-split` estimates a split depth from CPU
count and early branching.

## Pruning

Pruner profiles are selectable from the CLI and library API:

- `none`: exhaustive DFS guardrail for correctness comparisons.
- `reachability`: prune when the head cannot still reach the end.
- `dots`: reachability plus required black/colored node and edge dots.
- `triangles`: adds local triangle feasibility checks.
- `regions`: validates closed color/sun regions before the path is complete.
- `symmetry`: adds dual-head reachability for mirrored paths.
- `all`: default production profile.

`DfsStats` records explored nodes, pruned states, and generated parallel work
items. `SolverReport` exposes those values for CLI, GUI, and profile exports.

## Validation

`WitnessGraph::from_json` validates schema-level invariants before building the
graph: positive bounded dimensions, exactly one start/end, in-bounds node and
cell coordinates, adjacent edge endpoints, duplicate constraints, triangle
counts, color ranges, and tetris shape sanity.
