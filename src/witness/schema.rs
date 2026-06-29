use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct PuzzleJson {
    #[serde(default)]
    pub width: usize,
    #[serde(default)]
    pub height: usize,
    #[serde(default)]
    pub starts: Vec<[usize; 2]>,
    #[serde(default)]
    pub ends: Vec<[usize; 2]>,
    #[serde(default)]
    pub node_dots: Vec<[usize; 2]>,
    #[serde(default)]
    pub edge_dots: Vec<[[usize; 2]; 2]>,
    #[serde(default)]
    pub broken_edges: Vec<[[usize; 2]; 2]>,
    #[serde(default)]
    pub squares: Vec<SquareJson>,
    #[serde(default)]
    pub stars: Vec<StarJson>,
    #[serde(default)]
    pub triangles: Vec<TriangleJson>,
    #[serde(default)]
    pub tetris: Vec<TetrisJson>,
    #[serde(default)]
    pub sun_cells: Vec<SunJson>,
    #[serde(default)]
    pub eliminations: Vec<[usize; 2]>,
    #[serde(default)]
    pub colored_node_dots: Vec<ColoredDotJson>,
    #[serde(default)]
    pub colored_edge_dots: Vec<ColoredEdgeDotJson>,
    #[serde(default)]
    pub symmetry: Option<SymmetryKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum SymmetryKind {
    #[serde(rename = "x")]
    MirrorX,
    #[serde(rename = "y")]
    MirrorY,
    #[serde(rename = "xy")]
    MirrorXY,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SquareJson {
    pub pos: [usize; 2],
    pub color: u8,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct StarJson {
    pub pos: [usize; 2],
    pub color: u8,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct TriangleJson {
    pub pos: [usize; 2],
    pub count: u8,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct TetrisJson {
    pub pos: [usize; 2],
    pub shape: Vec<[i8; 2]>,
    #[serde(default)]
    pub negative: bool,
    #[serde(default = "default_true")]
    pub can_rotate: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SunJson {
    pub pos: [usize; 2],
    pub color: u8,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ColoredDotJson {
    pub pos: [usize; 2],
    pub color: u8,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ColoredEdgeDotJson {
    pub endpoints: [[usize; 2]; 2],
    pub color: u8,
}
