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
                let s = self.debug_char(dx, dy, s);
                print!("{}", s);
            }
            println!();
        }
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
}

impl WitnessState {
    pub fn draw(&self, g: &Graph) {
        g.draw_with_state(Some(self));
    }
}
