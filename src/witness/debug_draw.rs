use crate::witness::graph::{CellConstraint, WitnessGraph};
use crate::witness::state::{WitnessState, test_bit};

impl WitnessGraph {
    pub fn draw_with_state(&self, s: Option<&WitnessState>) {
        let w = self.width;
        let h = self.height;

        let mirror_used: Vec<u64> = if self.symmetry.is_some() {
            if let Some(st) = s {
                let mut mirror = vec![0u64; st.used_edges.len()];
                for (word_idx, &word) in st.used_edges.iter().enumerate() {
                    if word == 0 {
                        continue;
                    }
                    for bit in 0..64 {
                        if (word >> bit) & 1 == 1 {
                            let ei = word_idx * 64 + bit;
                            if let Some(me) = self.symmetric_edge(ei) {
                                let w = me >> 6;
                                let b = me & 63;
                                if w < mirror.len() {
                                    mirror[w] |= 1u64 << b;
                                }
                            }
                        }
                    }
                }
                mirror
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        for dy in 0..=2 * h {
            for dx in 0..=2 * w {
                print!("{}", self.debug_char(dx, dy, s, &mirror_used));
            }
            println!();
        }
        println!();
    }

    fn debug_char(
        &self,
        dx: usize,
        dy: usize,
        s: Option<&WitnessState>,
        mirror_used: &[u64],
    ) -> &str {
        match (dx & 1, dy & 1) {
            (0, 0) => self.draw_vertex(dx / 2, dy / 2, s),
            (1, 0) => self.draw_h_edge(dx / 2, dy / 2, s, mirror_used),
            (0, 1) => self.draw_v_edge(dx / 2, dy / 2, s, mirror_used),
            (1, 1) => self.draw_cell(dx / 2, dy / 2),
            _ => unreachable!(),
        }
    }

    fn draw_vertex(&self, x: usize, y: usize, s: Option<&WitnessState>) -> &str {
        let ni = self.node_xy_to_idx(x, y);

        if let Some(st) = s {
            if ni == st.head {
                return "H";
            }
        }
        if ni == self.start {
            return "S";
        }
        if ni == self.end {
            return "E";
        }
        if self.dot_nodes.contains(&ni) {
            return "o";
        }
        "+"
    }

    fn draw_h_edge(
        &self,
        x: usize,
        y: usize,
        s: Option<&WitnessState>,
        mirror_used: &[u64],
    ) -> &str {
        if x >= self.width {
            return " ";
        }
        let ei = self.h_edge_index(x, y);
        if self.is_broken(ei) {
            return " ";
        }
        let used = s.is_some_and(|st| test_bit(&st.used_edges, ei));
        if used {
            "="
        } else if !mirror_used.is_empty() && test_bit(mirror_used, ei) {
            ":"
        } else {
            "-"
        }
    }

    fn draw_v_edge(
        &self,
        x: usize,
        y: usize,
        s: Option<&WitnessState>,
        mirror_used: &[u64],
    ) -> &str {
        if y >= self.height {
            return " ";
        }
        let ei = self.v_edge_index(x, y);
        if self.is_broken(ei) {
            return " ";
        }
        let used = s.is_some_and(|st| test_bit(&st.used_edges, ei));
        if used {
            "#"
        } else if !mirror_used.is_empty() && test_bit(mirror_used, ei) {
            "$"
        } else {
            "|"
        }
    }

    fn draw_cell(&self, cx: usize, cy: usize) -> &str {
        if cx >= self.width || cy >= self.height {
            return " ";
        }
        match self.cell(cx, cy) {
            CellConstraint::None => ".",
            CellConstraint::Square { color } => match color {
                1 => "B", // black / color 1
                2 => "W", // white / color 2
                3 => "R", // red
                4 => "G", // green
                5 => "O", // orange
                _ => "S",
            },
            CellConstraint::Star { .. } => "*",
            CellConstraint::Triangle { count } => match count {
                1 => "1",
                2 => "2",
                3 => "3",
                _ => "T",
            },
            CellConstraint::Sun { .. } => "U",
            CellConstraint::Tetris { .. } => "T",
            CellConstraint::Elimination => "X",
        }
    }
}
