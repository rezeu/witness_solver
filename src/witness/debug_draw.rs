use crate::witness::graph::{CellConstraint, WitnessGraph};
use crate::witness::state::{test_bit, WitnessState};

impl WitnessGraph {
    pub fn draw_with_state(&self, s: Option<&WitnessState>) {
        let w = self.width;
        let h = self.height;

        for dy in 0..=2 * h {
            for dx in 0..=2 * w {
                print!("{}", self.debug_char(dx, dy, s));
            }
            println!();
        }
        println!();
    }

    fn debug_char(&self, dx: usize, dy: usize, s: Option<&WitnessState>) -> &str {
        match (dx & 1, dy & 1) {
            (0, 0) => self.draw_vertex(dx / 2, dy / 2, s),
            (1, 0) => self.draw_h_edge(dx / 2, dy / 2, s),
            (0, 1) => self.draw_v_edge(dx / 2, dy / 2, s),
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

    fn draw_h_edge(&self, x: usize, y: usize, s: Option<&WitnessState>) -> &str {
        if x >= self.width {
            return " ";
        }
        let ei = self.h_edge_index(x, y);
        if self.is_broken(ei) {
            return " ";
        }
        let used = s.map_or(false, |st| test_bit(&st.used_edges, ei));
        if used { "=" } else { "-" }
    }

    fn draw_v_edge(&self, x: usize, y: usize, s: Option<&WitnessState>) -> &str {
        if y >= self.height {
            return " ";
        }
        let ei = self.v_edge_index(x, y);
        if self.is_broken(ei) {
            return " ";
        }
        let used = s.map_or(false, |st| test_bit(&st.used_edges, ei));
        if used { "#" } else { "|" }
    }

    fn draw_cell(&self, cx: usize, cy: usize) -> &str {
        if cx >= self.width || cy >= self.height {
            return " ";
        }
        match self.cell(cx, cy) {
            CellConstraint::None => ".",
            CellConstraint::Square { color } => match color {
                1 => "B",  // black / color 1
                2 => "W",  // white / color 2
                3 => "R",  // red
                4 => "G",  // green
                5 => "O",  // orange
                _ => "S",
            },
            CellConstraint::Star { .. } => "*",
            CellConstraint::Triangle { count } => match count {
                1 => "1",
                2 => "2",
                3 => "3",
                _ => "T",
            },
            CellConstraint::Tetris { .. } => "T",
            CellConstraint::Elimination => "X",
        }
    }
}
