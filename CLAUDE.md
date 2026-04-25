# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A high-performance, modular Rust solver for The Witness puzzle game. Built around a generic parallel DFS framework that can be reused for other search problems.

## Build & Run

```bash
cargo build --release           # optimized build (LTO + single codegen unit)
cargo run --release -- <file>   # solve a puzzle from JSON
cargo run -- puzzle.json --seq  # force sequential (no parallelism)
cargo test                      # run all tests
```

Requires Rust nightly (edition 2024). Dependencies: `rayon`, `smallvec`, `serde`, `serde_json`.

## Architecture

### Generic Solver (`src/solver/`)

Problem-agnostic DFS framework. Any search problem can plug in by implementing the traits:

- **`state.rs`** — `SearchState` trait with associated types: `Ctx` (shared context like a graph), `Move`, `UndoEntry`. Methods: `gen_moves()`, `apply_move()`, `apply_undo()`.
- **`pruner.rs`** — `Pruner<S>` trait (`should_prune`). `PrunerChain` composes via short-circuit OR.
- **`satisfier.rs`** — `Satisfier<S>` trait (`is_satisfied`). `SatisfierChain` composes via short-circuit AND.
- **`undo.rs`** — `UndoStack<S>` with mark/rollback for backtracking. Uses `SmallVec` (inline capacity 64).
- **`dfs.rs`** — `run_dfs` (sequential) and `run_parallel_dfs` (rayon work-stealing). Parallel version expands `split_depth` levels to create work items, then processes in parallel with `AtomicBool` early termination.

All traits require `Send + Sync` for parallel safety. The context (`Ctx`) is shared immutable across threads; state is cloned per worker.

### Witness Problem (`src/witness/`)

- **`graph.rs`** — `WitnessGraph` loaded from JSON. Contains grid dimensions, start/end nodes, broken edges (bitset), dot nodes/edges (lists), cell constraints (`CellConstraint` enum: Square, Star, Triangle, Tetris, Elimination).
- **`state.rs`** — `WitnessState` implements `SearchState<Ctx=WitnessGraph>`. Tracks used edges as bitset (`Vec<u64>`), per-node degrees, head position. Moves are edge indices. `gen_moves` enforces: no revisiting nodes, no broken edges, stop at end.
- **`region.rs`** — Flood-fill computation of cell regions separated by the current path. Two cells are connected iff the grid edge between them is NOT used.
- **`rules.rs`** — `WitnessValidator` implements `Satisfier`. Checks in order: path complete → degree invariant → dots → triangles → (compute regions) → squares → stars → tetris. With elimination marks, uses per-region violation counting with elimination pairing. Region computation is deferred until needed.
- **`pruners.rs`** — `ReachabilityPruner`: BFS from head to end through unvisited nodes. `DotReachabilityPruner`: also checks dot nodes reachable. `TrianglePruner`: early detection of impossible triangle constraints. Uses stack-allocated `[u64; 4]` bitset and `[usize; 289]` stack (supports up to 16x16 grids). Pre-computed adjacency list eliminates division/modulo in hot paths.
- **`debug_draw.rs`** — ASCII visualization. `=`/`#` for used horizontal/vertical edges, `H` for head, `S`/`E` for start/end, `o` for dots, `B`/`W` for squares.

### Edge Indexing (critical to understand)

Interleaved encoding: `edge_index = real_index << 1 | direction_bit`. Even = horizontal, odd = vertical.
- `h_edge_index(x, y) = 2 * (y * width + x)` — horizontal edge from node (x,y) to (x+1,y)
- `v_edge_index(x, y) = 2 * (y * (width+1) + x) + 1` — vertical edge from node (x,y) to (x,y+1)

Nodes indexed as `y * (width + 1) + x`. Cells indexed as `cy * width + cx`.

### JSON Puzzle Format

```json
{
  "width": 4, "height": 4,
  "starts": [[0, 4]], "ends": [[4, 0]],
  "node_dots": [[2, 2]],
  "edge_dots": [[[0, 0], [1, 0]]],
  "broken_edges": [[[1, 1], [1, 2]]],
  "squares": [{"pos": [0, 0], "color": 1}],
  "stars": [{"pos": [2, 2], "color": 1}],
  "triangles": [{"pos": [0, 1], "count": 2}],
  "tetris": [{"pos": [1, 1], "shape": [[0,0],[1,0],[0,1]], "negative": false}],
  "eliminations": [[3, 3]]
}
```

All fields except width/height/starts/ends are optional. Coordinates: nodes are (x,y) in grid space, cells are (cx,cy) in cell space.

## Implemented vs Stubbed

**Working**: line path, dots, colored squares, stars, triangles, tetris/polyomino tiling (positive and negative), elimination marks, reachability pruning, triangle early pruning, closed-region early pruning, parallel DFS with profiling (`--profile` flag)
**Stubbed (TODO)**: symmetry puzzles

## Performance

- DFS with rayon work-stealing; `split_depth` controls parallelism granularity
- Pre-computed adjacency list, reusable moves buffer, stack-allocated pruner BFS
- `ClosedRegionPruner` validates square/star color constraints on regions that can no longer be split, instead of waiting for path completion — the dominant win on low-density square puzzles
- `--profile` flag benchmarks sequential vs parallel at multiple split depths
- Full integration test suite (15 puzzles): ~5s parallel
- `stress_7x7` (7x7, low-density squares): ~1.2s parallel; ~13min sequential (parallel wins via work-stealing finding the solution faster)
- `stress_mixed_6x6` and `hard_6x6`: sub-second parallel
