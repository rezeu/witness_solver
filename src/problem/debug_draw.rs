use crate::problem::{
    graph::Graph,
    witness_state::{WitnessState, test_bit},
};

impl Graph {
    pub fn draw_with_state(&self, s: Option<&WitnessState>) {
        let w = self.width;
        let h = self.height;

        for dy in 0..=2 * h {
            for dx in 0..=2 * w {
                let str = self.debug_char(dx, dy, s);
                print!("{}", str);
            }
            println!();
        }
        println!();
    }
    fn debug_char(&self, dx: usize, dy: usize, s: Option<&WitnessState>) -> &str {
        match (dx % 2, dy % 2) {
            (0, 0) => self.debug_vertex(dx / 2, dy / 2, s),
            (1, 0) => self.debug_h_edge(dx / 2, dy / 2, s),
            (0, 1) => self.debug_v_edge(dx / 2, dy / 2, s),
            (1, 1) => self.debug_cell(dx / 2, dy / 2),
            _ => unreachable!(),
        }
    }
    fn debug_vertex(&self, x: usize, y: usize, s: Option<&WitnessState>) -> &str {
        let v = y * (self.width + 1) + x;

        if let Some(st) = s {
            if v == st.head {
                "H"
            } else if v == self.start {
                "S"
            } else if v == self.end {
                "E"
            } else {
                "+"
            }
        } else {
            "+"
        }
    }
    fn debug_h_edge(&self, x: usize, y: usize, st: Option<&WitnessState>) -> &str {
        if !self.has_h_edge(x, y) {
            return " ".into();
        }

        let used = st.map_or(false, |st| {
            let ei = self.h_edge_index(x, y);
            test_bit(&st.used_edges, ei)
        });

        if used { "=".into() } else { "-".into() }
    }
    fn debug_v_edge(&self, x: usize, y: usize, st: Option<&WitnessState>) -> &str {
        if !self.has_v_edge(x, y) {
            return " ".into();
        }

        let used = st.map_or(false, |st| {
            let ei = self.v_edge_index(x, y);
            test_bit(&st.used_edges, ei)
        });

        if used { "#".into() } else { "|".into() }
    }
    fn debug_cell(&self, x: usize, y: usize) -> &str {
        // // 预留：颜色、消除块、tetris、星星等
        // if let Some(rule) = self.cell_rule(x, y) {
        //     rule.debug_char()
        // } else {
        //     ".".into()
        // }
        ".".into()
    }
}
