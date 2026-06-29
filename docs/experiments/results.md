# Experiment Results

Generated on 2026-06-29 with `cargo run --release -- <puzzle> --profile --pruners all`
on an 8 logical CPU WSL environment.

| Puzzle | Sequential | Best Parallel | Split | Speedup | Seq Nodes | Best Par Nodes | Best Par Pruned | Work Items | Dominant Pruner Hits |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `basic_4x4` | 0.000012s | 0.000280s | 3 | 0.04x | 9 | 13 | 0 | 10 | reachability: 0 |
| `dots_3x3` | 0.000014s | 0.000246s | 3 | 0.06x | 12 | 16 | 0 | 10 | dots: 0 |
| `hard_6x6` | 11.673s | 3.952s | 4 | 2.95x | 12,855,257 | 20,308,959 | 9,989,519 | 24 | dots: 6,138,381; triangles: 300,446; regions: 3,550,692 |

Small puzzles are intentionally dominated by Rayon setup and split overhead.
`hard_6x6` is the useful demonstration case here: split depth 4 produced the
best measured wall-clock time and shows that region and dot pruning account for
most rejected states.

Raw data:

- [basic_4x4.csv](data/basic_4x4.csv)
- [dots_3x3.csv](data/dots_3x3.csv)
- [hard_6x6.csv](data/hard_6x6.csv)
