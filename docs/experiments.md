# Experiments

Use release builds for all measurements.

```bash
cargo run --release -- puzzles/basic_4x4.json --profile --profile-json /tmp/basic.json --profile-csv /tmp/basic.csv
cargo run --release -- puzzles/hard_6x6.json --profile --profile-json /tmp/hard6.json --profile-csv /tmp/hard6.csv
```

Do not run `--profile` on `stress_7x7` casually: profile mode includes a
sequential pass. Use `cargo test --release -- --ignored` for the stress fixture.

For pruning ablations:

```bash
for p in none reachability dots triangles regions symmetry all; do
  cargo run --release -- puzzles/basic_4x4.json --profile --pruners "$p" --profile-csv "/tmp/basic-$p.csv"
done
```

Recommended report columns:

- puzzle name
- pruner profile
- sequential time
- best parallel split depth
- best parallel time
- explored nodes
- pruned states
- per-pruner hit counts
- work items
- throughput
- speedup

The CLI prints the human-readable profile and can export the same run data as
JSON or CSV for tables and plots.

Generated sample data lives in `docs/experiments/data/`:

- `basic_4x4.json` / `basic_4x4.csv`
- `dots_3x3.json` / `dots_3x3.csv`
- `hard_6x6.json` / `hard_6x6.csv`

See [Results](experiments/results.md) for a compact table derived from those
runs.
