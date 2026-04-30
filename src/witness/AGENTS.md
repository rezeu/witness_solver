# src/witness — Witness Puzzle Logic

## OVERVIEW
Witness-specific puzzle representation, constraint validation, pruners, and CLI entry point. All JSON loading, ASCII rendering, and region analysis lives here.

## STRUCTURE
```
src/witness/
├── mod.rs          # Library API (build_pruner_chain, solve, run_profile_bench, ProfileReport)
├── graph.rs        # WitnessGraph: grid, edges, cell constraints from JSON
├── state.rs        # WitnessState: path tracking via used-edge bitset
├── rules.rs        # WitnessValidator: all constraint checks in order
├── pruners.rs      # ReachabilityPruner, DotReachabilityPruner, TrianglePruner, ClosedRegionPruner
├── region.rs       # Flood-fill cell region computation
└── debug_draw.rs   # ASCII puzzle renderer
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Load puzzle from JSON | `graph.rs::from_file()` | Deserializes `PuzzleJson` struct |
| Edge index conversion | `graph.rs::h_edge_index()`, `v_edge_index()` | Interleaved: even=horizontal, odd=vertical |
| Path state tracking | `state.rs::WitnessState` | `used_edges: Vec<u64>` bitset, node degrees, head position |
| Move generation | `state.rs::gen_moves()` | Directions from current head, filters visited/broken |
| Constraint validation | `rules.rs::WitnessValidator::check()` | Ordered: path complete → degree → dots → triangles → squares → stars → tetris |
| Reachability pruning | `pruners.rs::ReachabilityPruner` | BFS from head to end via unvisited nodes |
| Dot pruning | `pruners.rs::DotReachabilityPruner` | Also checks dot reachability |
| Triangle pruning | `pruners.rs::TrianglePruner` | Early triangle violation detection |
| Closed region pruning | `pruners.rs::ClosedRegionPruner` | Validates color constraints when regions lock |
| Region computation | `region.rs` | Flood-fill cells; two cells connected iff edge between them NOT used |
| ASCII debug | `debug_draw.rs` | `=`/`#` for edges, `H` for head, `S`/`E` for start/end |

## CONVENTIONS
- **Edge encoding**: `edge_index = real_index << 1 | direction_bit`. Even = horizontal, odd = vertical.
- **Node indexing**: `y * (width + 1) + x`. Cell indexing: `cy * width + cx`.
- **Bitset storage**: `Vec<u64>` for path edges (scale with grid), `[u64; 4]` for pruner BFS bitsets (stack-allocated).
- **Pruner chain order**: Reachability → DotReachability → Triangle → ClosedRegion (short-circuit OR).
- **Region deferred**: Region computation only triggered when needed (after basic checks pass).
- **Elimination marks**: Per-region violation counting with elimination pairing allows negative constraints.
- **Stack-allocated BFS**: `[u64; 4]` visited set and `[usize; 289]` queue support up to 16×16 grids in pruners.
- **CLI in main.rs**: `main.rs` uses clap derive; library provides `solve()`, `build_pruner_chain()`, `run_profile_bench()`, `ProfileReport`.

## ANTI-PATTERNS
- **DO NOT** break the constraint check order in `WitnessValidator::check()` — region-dependent checks expect earlier filters.
- **DO NOT** change edge index parity convention — even=horizontal, odd=vertical is assumed everywhere.
- **DO NOT** remove `#[inline(always)]` on bitset helpers in `state.rs` and `pruners.rs` without benchmarking.
