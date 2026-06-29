# Puzzle JSON Format

All fields except `width`, `height`, `starts`, and `ends` are optional arrays.
Coordinates are zero-based.

```json
{
  "width": 4,
  "height": 4,
  "starts": [[0, 4]],
  "ends": [[4, 0]],
  "node_dots": [[2, 2]],
  "edge_dots": [[[0, 4], [1, 4]]],
  "broken_edges": [[[1, 1], [1, 2]]],
  "squares": [{"pos": [0, 0], "color": 1}],
  "stars": [{"pos": [1, 0], "color": 1}],
  "sun_cells": [{"pos": [2, 0], "color": 1}],
  "triangles": [{"pos": [1, 3], "count": 2}],
  "tetris": [
    {
      "pos": [0, 0],
      "shape": [[0, 0], [1, 0]],
      "negative": false,
      "can_rotate": true
    }
  ],
  "eliminations": [[3, 3]],
  "colored_node_dots": [{"pos": [1, 1], "color": 1}],
  "colored_edge_dots": [{"endpoints": [[0, 0], [1, 0]], "color": 1}],
  "symmetry": "x"
}
```

## Coordinate Domains

- Node coordinates: `0 <= x <= width`, `0 <= y <= height`.
- Cell coordinates: `0 <= x < width`, `0 <= y < height`.
- Edge endpoints must be adjacent grid nodes.
- `symmetry` may be `"x"`, `"y"`, `"xy"`, or omitted.

## Validation Rules

The loader rejects malformed puzzle data instead of silently constructing a bad
graph. Examples include empty dimensions, multiple starts or ends, out-of-bounds
coordinates, duplicate node/edge/cell constraints, edge dots on broken edges,
invalid triangle counts, invalid color ids, disconnected tetris shapes, and
tetris shapes that cannot fit the grid.
