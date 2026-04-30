use crate::witness::graph::WitnessGraph;
#[cfg(test)]
use crate::witness::graph::PuzzleJson;
use crate::witness::state::WitnessState;

pub struct RegionMap {
    pub labels: Vec<u8>,  // region id per cell, indexed as cy * width + cx
    pub count: u8,
    width: usize,
}

impl RegionMap {
    #[inline]
    pub fn cell_region(&self, cx: usize, cy: usize) -> u8 {
        self.labels[cy * self.width + cx]
    }

    pub fn cells_in_region(&self, region: u8) -> impl Iterator<Item = (usize, usize)> + '_ {
        let w = self.width;
        self.labels
            .iter()
            .enumerate()
            .filter(move |&(_, r)| *r == region)
            .map(move |(i, _)| (i % w, i / w))
    }
}

/// Flood-fill cells into connected regions.
/// Two adjacent cells are connected iff the grid edge between them is NOT used.
pub fn compute_regions(s: &WitnessState, g: &WitnessGraph) -> RegionMap {
    let w = g.width;
    let h = g.height;
    let mut labels = vec![0xFFu8; w * h];
    let mut region_id: u8 = 0;
    let mut stack = Vec::with_capacity(w * h);

    for start_y in 0..h {
        for start_x in 0..w {
            if labels[start_y * w + start_x] != 0xFF {
                continue;
            }

            labels[start_y * w + start_x] = region_id;
            stack.push((start_x, start_y));

            while let Some((cx, cy)) = stack.pop() {
                // Right: cell (cx+1, cy) — separated by v_edge(cx+1, cy)
                if cx + 1 < w {
                    let idx = cy * w + cx + 1;
                    if labels[idx] == 0xFF && !s.used(g.v_edge_index(cx + 1, cy)) {
                        labels[idx] = region_id;
                        stack.push((cx + 1, cy));
                    }
                }
                // Left: cell (cx-1, cy) — separated by v_edge(cx, cy)
                if cx > 0 {
                    let idx = cy * w + cx - 1;
                    if labels[idx] == 0xFF && !s.used(g.v_edge_index(cx, cy)) {
                        labels[idx] = region_id;
                        stack.push((cx - 1, cy));
                    }
                }
                // Down: cell (cx, cy+1) — separated by h_edge(cx, cy+1)
                if cy + 1 < h {
                    let idx = (cy + 1) * w + cx;
                    if labels[idx] == 0xFF && !s.used(g.h_edge_index(cx, cy + 1)) {
                        labels[idx] = region_id;
                        stack.push((cx, cy + 1));
                    }
                }
                // Up: cell (cx, cy-1) — separated by h_edge(cx, cy)
                if cy > 0 {
                    let idx = (cy - 1) * w + cx;
                    if labels[idx] == 0xFF && !s.used(g.h_edge_index(cx, cy)) {
                        labels[idx] = region_id;
                        stack.push((cx, cy - 1));
                    }
                }
            }

            region_id += 1;
        }
    }

    RegionMap {
        labels,
        count: region_id,
        width: w,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph(w: usize, h: usize) -> WitnessGraph {
        let json = PuzzleJson {
            width: w,
            height: h,
            starts: vec![[0, 0]],
            ends: vec![[w, h]],
            symmetry: None,
            node_dots: vec![],
            edge_dots: vec![],
            broken_edges: vec![],
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
        };
        WitnessGraph::from_json(json).unwrap()
    }

    fn set_used(state: &mut WitnessState, ei: usize) {
        let w = ei >> 6;
        let b = ei & 63;
        state.used_edges[w] |= 1u64 << b;
    }

    #[test]
    fn test_all_same_region() {
        let g = make_graph(3, 3);
        let s = WitnessState::new(&g);
        let regions = compute_regions(&s, &g);
        assert_eq!(regions.count, 1);
        for &label in &regions.labels {
            assert_eq!(label, 0);
        }
    }

    #[test]
    fn test_path_creates_two_regions() {
        let g = make_graph(3, 3);
        let mut s = WitnessState::new(&g);
        set_used(&mut s, g.v_edge_index(1, 0));
        set_used(&mut s, g.v_edge_index(1, 1));
        set_used(&mut s, g.v_edge_index(1, 2));
        let regions = compute_regions(&s, &g);
        assert_eq!(regions.count, 2);
        let left_count = regions.cells_in_region(0).count();
        let right_count = regions.cells_in_region(1).count();
        assert_eq!(left_count, 3);
        assert_eq!(right_count, 6);
    }

    #[test]
    fn test_used_edge_separates_cells() {
        let g = make_graph(2, 2);
        let mut s = WitnessState::new(&g);
        set_used(&mut s, g.h_edge_index(0, 1));
        set_used(&mut s, g.v_edge_index(1, 0));
        set_used(&mut s, g.v_edge_index(1, 1));
        let regions = compute_regions(&s, &g);
        assert_ne!(regions.cell_region(0, 0), regions.cell_region(0, 1));
    }
}
