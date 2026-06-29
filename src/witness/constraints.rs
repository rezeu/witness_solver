#[derive(Default, Clone, Debug)]
pub enum CellConstraint {
    #[default]
    None,
    Square {
        color: u8,
    },
    Star {
        color: u8,
    },
    Sun {
        color: u8,
    },
    Triangle {
        count: u8,
    },
    Tetris {
        shape: Vec<[i8; 2]>,
        negative: bool,
        can_rotate: bool,
    },
    Elimination,
}
