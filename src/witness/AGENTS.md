# src/witness — Witness Puzzle Logic

## OVERVIEW

This directory contains Witness-specific model construction, validation,
pruning, rendering, and public library entry points. Generic DFS machinery lives
in `src/solver/`; GUI code lives in `src/gui.rs`.

## STRUCTURE

```
src/witness/
├── mod.rs          # Public API, pruner profiles, solve/profile orchestration
├── schema.rs       # serde JSON schema structs
├── constraints.rs  # CellConstraint enum
├── types.rs        # NodeId, EdgeId, CellPos, NodePos aliases
├── indexing.rs     # Pure node/edge indexing formulas
├── graph.rs        # Validated WitnessGraph and topology caches
├── state.rs        # WitnessState, move generation, apply/undo
├── rules.rs        # WitnessValidator and rule checks
├── pruners.rs      # Reachability/dot/triangle/region/symmetry pruners
├── region.rs       # Flood-fill region computation
└── debug_draw.rs   # ASCII puzzle renderer
```

## WHERE TO LOOK

| Task | Location | Notes |
|---|---|---|
| Add JSON field | `schema.rs`, then `graph.rs::validate_json()` / `from_json()` | Validate before indexing |
| Add cell rule type | `constraints.rs`, `graph.rs`, `rules.rs`, GUI if editable | Preserve validation order |
| Add pruner | `pruners.rs`, then `mod.rs::build_pruner_chain_with_profile()` | Give it a stable name for hit stats |
| Change pruner profiles | `mod.rs::PrunerProfile` | Update CLI/docs/tests |
| Change DFS parallel behavior | `src/solver/dfs.rs` | Witness layer passes split depth/config |
| Edge indexing logic | `indexing.rs` | Keep even=horizontal, odd=vertical |
| Move ordering | `state.rs::gen_moves()` | Sort only moves appended for the current state |
| Final validation | `rules.rs::WitnessValidator::is_satisfied()` | Path complete → degrees → dots → local rules → regions |
| Input validation tests | `tests/validation.rs` | Invalid JSON should return `GraphError`, not panic |
| Fixture regression tests | `tests/puzzles.rs` | Solvable/unsolvable and seq/par consistency |

## IMPORTANT INVARIANTS

- `WitnessGraph` is immutable after construction. Fields are `pub(crate)`; use
  public accessor methods from outside the crate.
- All puzzle schema data must be validated before graph construction indexes
  into vectors or computes edge ids.
- Node coordinates are `0..=width` by `0..=height`; cell coordinates are
  `0..width` by `0..height`.
- `EdgeId` parity is semantic: even = horizontal, odd = vertical.
- `WitnessState` uses `UndoStack` for in-place DFS backtracking.
- Symmetry moves may set both a player edge and mirror edge; self-symmetric
  edges must be counted once.
- Region computation is deferred until needed by final validation or closed
  region pruning.
- Pruner chain order is profile-dependent and short-circuits on first hit.
  `PrunerChain` records per-pruner hit counts for reports.

## PUBLIC API IN THIS MODULE

- `load_puzzle(path)`
- `solve_puzzle(graph, SolverConfig)`
- `profile_puzzle(graph, SolverConfig)`
- `SolverConfig`
- `SolverReport`
- `PrunerProfile`
- Existing lower-level helpers: `build_pruner_chain`, `solve`,
  `run_profile_bench`

## PRUNER PROFILES

- `none`
- `reachability`
- `dots`
- `triangles`
- `regions`
- `symmetry`
- `all`

When adding or changing a profile, update:

- `PrunerProfile` parsing/display in `mod.rs`
- CLI help in `src/main.rs` if needed
- API tests in `tests/api.rs`
- README/docs references

## ANTI-PATTERNS

- Do not put serde schema definitions back into `graph.rs`; keep `schema.rs`
  as the single schema source.
- Do not duplicate edge index formulas outside `indexing.rs`.
- Do not skip `validate_json()` when constructing `WitnessGraph`.
- Do not make `WitnessGraph` fields public again without a clear API reason.
- Do not add a pruner without a stable name in `PrunerChain`; profile exports
  depend on those names.
- Do not run expensive region/tetris checks before cheap path/degree/dot checks
  unless a benchmark justifies it.
