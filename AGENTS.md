# PROJECT KNOWLEDGE BASE

**Updated:** 2026-06-29
**Project:** `witness_solver`
**Branch:** main

## OVERVIEW

High-performance Rust solver for Witness-style line puzzles. A generic DFS
framework in `src/solver/` powers Witness-specific graph construction,
constraint validation, pruning, profiling, and GUI visualization in
`src/witness/` and `src/gui.rs`.

The project uses Rust edition 2024 with `rust-toolchain.toml` pinned to nightly.

## STRUCTURE

```
./
├── Cargo.toml                 # edition 2024, release LTO + codegen-units=1
├── rust-toolchain.toml        # nightly
├── README.md                  # user-facing project/report entry
├── docs/                      # algorithm notes, JSON format, experiments, screenshots
├── puzzles/                   # JSON puzzle fixtures
├── src/
│   ├── main.rs                # clap CLI, GUI launcher, profile export wiring
│   ├── lib.rs                 # pub mod gui; solver; witness;
│   ├── gui.rs                 # egui editor, solve stats, replay animation
│   ├── solver/                # generic DFS framework
│   └── witness/               # Witness model, validation, pruning, rendering
└── tests/
    ├── api.rs                 # public API/config/report tests
    ├── puzzles.rs             # fixture solve/regression matrix
    ├── proptest.rs            # apply/undo and move-generation properties
    └── validation.rs          # malformed puzzle validation tests
```

## WHERE TO LOOK

| Task | Location | Notes |
|---|---|---|
| Public solver API | `src/witness/mod.rs` | `load_puzzle`, `solve_puzzle`, `profile_puzzle`, `SolverConfig`, `SolverReport` |
| CLI flags | `src/main.rs` | clap derive; supports `--seq`, `--gui`, `--profile`, `--split-depth`, `--auto-split`, `--pruners`, exports |
| GUI editor/demo | `src/gui.rs` | `EditablePuzzle`, solve stats panel, solution replay animation |
| JSON schema | `src/witness/schema.rs` | serde-facing puzzle format structs |
| Graph validation/topology | `src/witness/graph.rs` | `WitnessGraph::from_json()`, immutable graph construction |
| Edge/node formulas | `src/witness/indexing.rs` | Pure indexing helpers; even edges horizontal, odd edges vertical |
| Cell constraints | `src/witness/constraints.rs` | `CellConstraint` enum |
| Semantic aliases | `src/witness/types.rs` | `NodeId`, `EdgeId`, `CellPos`, `NodePos` |
| Path state and move ordering | `src/witness/state.rs` | `WitnessState`, apply/undo, deterministic move ordering |
| Constraint validation | `src/witness/rules.rs` | `WitnessValidator` and per-rule checks |
| Pruners | `src/witness/pruners.rs` | reachability, dots, triangles, closed regions, symmetry |
| DFS stats/pruner hit counts | `src/solver/dfs.rs`, `src/solver/pruner.rs` | nodes, pruned states, work items, per-pruner hits |
| Region computation | `src/witness/region.rs` | flood-fill cells separated by used path edges |
| ASCII rendering | `src/witness/debug_draw.rs` | CLI puzzle visualization |
| Experiment data | `docs/experiments/` | CSV/JSON profile outputs and result summary |

## PUBLIC COMMANDS

```bash
cargo check
cargo build --release
cargo run --release -- puzzles/basic_4x4.json
cargo run --release -- puzzles/basic_4x4.json --seq
cargo run --release -- puzzles/basic_4x4.json --auto-split --pruners all
cargo run --release -- puzzles/hard_6x6.json --profile --profile-json /tmp/profile.json --profile-csv /tmp/profile.csv
cargo run --release -- --gui
cargo test --release
cargo test --release -- --ignored
```

`cargo run --release` without arguments opens the GUI by default.

## CONVENTIONS

- **Release tests are the baseline.** DFS-heavy tests are intentionally run with `cargo test --release`.
- **CLI uses clap.** Do not reintroduce manual argument parsing.
- **Primary library API:** use `load_puzzle`, `solve_puzzle`, and `profile_puzzle` for new integrations.
- **Pruner profiles:** `none`, `reachability`, `dots`, `triangles`, `regions`, `symmetry`, `all`.
- **Profile exports:** JSON and CSV include nodes, pruned states, work items, split depth, pruner profile, and per-pruner hit counts.
- **Graph immutability:** `WitnessGraph` is constructed through validation and then treated as immutable. Fields are crate-visible; external code should use read-only accessors.
- **Edge encoding:** even edge ids are horizontal; odd edge ids are vertical. Keep this invariant intact.
- **GUI puzzle conversion:** keep `EditablePuzzle` ↔ `PuzzleJson` conversion centralized in `src/gui.rs` `From` impls.
- **Experiment artifacts:** update `docs/experiments/results.md` and `docs/experiments/data/` when collecting new profile numbers.

## TESTING EXPECTATIONS

- For solver/graph/rule changes, run `cargo test --release`.
- For stress verification, run `cargo test --release -- --ignored`.
- For CLI/profile changes, run at least:

```bash
cargo run --release -- puzzles/minimal_1x1.json --seq
cargo run --release -- puzzles/minimal_1x1.json --auto-split --pruners all
cargo run --release -- puzzles/minimal_1x1.json --profile --profile-json /tmp/witness_profile.json --profile-csv /tmp/witness_profile.csv --pruners all
```

## ANTI-PATTERNS

- Do not change release `lto=true` or `codegen-units=1` without benchmarking.
- Do not weaken puzzle input validation to preserve malformed fixtures.
- Do not bypass `UndoStack` with clone-heavy DFS unless measuring the impact.
- Do not remove per-pruner hit accounting from `PrunerChain`; reports and docs depend on it.
- Do not run `--profile` on `stress_7x7` casually, because profile mode includes a sequential pass.
- Do not add suppressions or broad `#[allow(...)]` attributes to hide type or lint issues.

## CURRENT CAPABILITIES

- Rules: starts/ends, broken edges, black and colored node/edge dots, squares,
  stars, suns, triangles, positive/negative tetris, eliminations, and `x`/`y`/`xy`
  mirror symmetry.
- Parallel DFS with configurable split depth and `--auto-split`.
- Deterministic move ordering.
- GUI solve statistics and solution replay.
- Profile JSON/CSV exports and checked-in sample experiment data.
