use eframe::egui;
use std::time::{Duration, Instant};

use crate::witness::graph::WitnessGraph;
use crate::witness::graph::{
    CellConstraint, ColoredDotJson, ColoredEdgeDotJson, PuzzleJson, SquareJson, StarJson, SunJson,
    SymmetryKind, TetrisJson, TriangleJson,
};
use crate::witness::state::WitnessState;
use crate::witness::{PrunerProfile, SolverConfig, SolverReport, solve_puzzle as solve_graph};

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

mod colors {
    use eframe::egui::Color32;

    pub const APP_BG: Color32 = Color32::from_rgb(34, 37, 36);
    pub const TOP_BAR: Color32 = Color32::from_rgb(42, 46, 45);
    pub const SIDEBAR: Color32 = Color32::from_rgb(39, 43, 42);
    pub const SIDEBAR_STROKE: Color32 = Color32::from_rgb(66, 72, 70);
    pub const CONTROL_BG: Color32 = Color32::from_rgb(52, 58, 56);
    pub const CONTROL_ACTIVE: Color32 = Color32::from_rgb(84, 108, 105);

    pub const CANVAS_BG: Color32 = Color32::from_rgb(31, 34, 33);
    pub const BOARD_BG: Color32 = Color32::from_rgb(91, 108, 102);
    pub const BOARD_RIM: Color32 = Color32::from_rgb(23, 28, 28);
    pub const CHANNEL: Color32 = Color32::from_rgb(20, 27, 28);
    pub const CHANNEL_SHADOW: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 70);
    pub const CHANNEL_HILITE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 22);

    pub const PATH: Color32 = Color32::from_rgb(255, 214, 51);
    pub const PATH_CORE: Color32 = Color32::from_rgb(255, 238, 106);
    pub const PATH_GLOW: Color32 = Color32::from_rgba_premultiplied(255, 214, 51, 82);
    pub const MIRROR_PATH: Color32 = Color32::from_rgba_premultiplied(92, 221, 213, 150);

    pub const HOVER_RING: Color32 = Color32::from_rgba_premultiplied(139, 202, 255, 145);
    pub const HOVER_FILL: Color32 = Color32::from_rgba_premultiplied(139, 202, 255, 28);

    pub const TEXT: Color32 = Color32::from_rgb(235, 239, 236);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(170, 179, 174);
    pub const TEXT_DARK: Color32 = Color32::from_rgb(18, 22, 22);

    pub const SQUARE_BLACK: Color32 = Color32::from_rgb(17, 21, 21);
    pub const SQUARE_WHITE: Color32 = Color32::from_rgb(235, 236, 228);
    pub const RED: Color32 = Color32::from_rgb(227, 77, 77);
    pub const GREEN: Color32 = Color32::from_rgb(62, 181, 112);
    pub const ORANGE: Color32 = Color32::from_rgb(238, 139, 45);
    pub const TRIANGLE: Color32 = Color32::from_rgb(245, 142, 54);
    pub const TETRIS: Color32 = Color32::from_rgb(242, 187, 51);
    pub const TETRIS_NEG: Color32 = Color32::from_rgb(155, 128, 232);
    pub const ELIMINATION: Color32 = Color32::from_rgb(203, 209, 202);
    pub const SYMBOL_STROKE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 95);

    pub const DOT_BLACK: Color32 = Color32::from_rgb(12, 16, 16);
    pub const DOT_BLUE: Color32 = Color32::from_rgb(75, 151, 255);
    pub const DOT_YELLOW: Color32 = Color32::from_rgb(250, 214, 62);
    pub const DOT_CYAN: Color32 = Color32::from_rgb(50, 215, 217);
    pub const DOT_RING: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 125);

    pub const SUCCESS_TEXT: Color32 = Color32::from_rgb(116, 229, 158);
    pub const ERROR_TEXT: Color32 = Color32::from_rgb(252, 132, 132);
}

fn dot_color32(color: u8) -> egui::Color32 {
    match color {
        0 => colors::DOT_BLACK,
        1 => colors::DOT_BLUE,
        2 => colors::DOT_YELLOW,
        3 => colors::DOT_CYAN,
        _ => colors::DOT_BLACK,
    }
}

fn symbol_color32(color: u8) -> egui::Color32 {
    match color {
        1 => colors::SQUARE_BLACK,
        2 => colors::SQUARE_WHITE,
        3 => colors::RED,
        4 => colors::GREEN,
        5 => colors::ORANGE,
        _ => colors::TEXT,
    }
}

fn color_name(color: u8) -> &'static str {
    match color {
        1 => "Black",
        2 => "White",
        3 => "Red",
        4 => "Green",
        5 => "Orange",
        _ => "Unknown",
    }
}

// ---------------------------------------------------------------------------
// Tool enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tool {
    Start,
    End,
    Square,
    Star,
    Sun,
    Triangle,
    Tetris,
    Elimination,
    BrokenEdge,
    NodeDot,
    EdgeDot,
    Eraser,
}

impl Tool {
    fn name(self) -> &'static str {
        match self {
            Tool::Start => "Start",
            Tool::End => "End",
            Tool::Square => "Square",
            Tool::Star => "Star",
            Tool::Sun => "Sun",
            Tool::Triangle => "Triangle",
            Tool::Tetris => "Tetris",
            Tool::Elimination => "Elimination",
            Tool::BrokenEdge => "Broken edge",
            Tool::NodeDot => "Node dot",
            Tool::EdgeDot => "Edge dot",
            Tool::Eraser => "Eraser",
        }
    }

    fn uses_cell_color(self) -> bool {
        matches!(self, Tool::Square | Tool::Star | Tool::Sun)
    }

    fn uses_dot_color(self) -> bool {
        matches!(self, Tool::NodeDot | Tool::EdgeDot)
    }
}

// ---------------------------------------------------------------------------
// Editable puzzle state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct EditablePuzzle {
    width: usize,
    height: usize,
    starts: Vec<[usize; 2]>,
    ends: Vec<[usize; 2]>,
    symmetry: Option<SymmetryKind>,
    squares: Vec<SquareJson>,
    stars: Vec<StarJson>,
    triangles: Vec<TriangleJson>,
    tetris: Vec<TetrisJson>,
    eliminations: Vec<[usize; 2]>,
    broken_edges: Vec<[[usize; 2]; 2]>,
    node_dots: Vec<[usize; 2]>,
    edge_dots: Vec<[[usize; 2]; 2]>,
    colored_node_dots: Vec<([usize; 2], u8)>,
    colored_edge_dots: Vec<([[usize; 2]; 2], u8)>,
    suns: Vec<SunJson>,
}

impl Default for EditablePuzzle {
    fn default() -> Self {
        Self {
            width: 4,
            height: 4,
            starts: vec![[0, 0]],
            ends: vec![[4, 4]],
            symmetry: None,
            squares: vec![],
            stars: vec![],
            triangles: vec![],
            tetris: vec![],
            eliminations: vec![],
            broken_edges: vec![],
            node_dots: vec![],
            edge_dots: vec![],
            colored_node_dots: vec![],
            colored_edge_dots: vec![],
            suns: vec![],
        }
    }
}

impl From<&EditablePuzzle> for PuzzleJson {
    fn from(puzzle: &EditablePuzzle) -> Self {
        PuzzleJson {
            width: puzzle.width,
            height: puzzle.height,
            starts: puzzle.starts.clone(),
            ends: puzzle.ends.clone(),
            node_dots: puzzle.node_dots.clone(),
            edge_dots: puzzle.edge_dots.clone(),
            broken_edges: puzzle.broken_edges.clone(),
            squares: puzzle.squares.clone(),
            stars: puzzle.stars.clone(),
            triangles: puzzle.triangles.clone(),
            tetris: puzzle.tetris.clone(),
            eliminations: puzzle.eliminations.clone(),
            symmetry: puzzle.symmetry,
            sun_cells: puzzle.suns.clone(),
            colored_node_dots: puzzle
                .colored_node_dots
                .iter()
                .map(|(pos, color)| ColoredDotJson {
                    pos: *pos,
                    color: *color,
                })
                .collect(),
            colored_edge_dots: puzzle
                .colored_edge_dots
                .iter()
                .map(|(endpoints, color)| ColoredEdgeDotJson {
                    endpoints: *endpoints,
                    color: *color,
                })
                .collect(),
        }
    }
}

impl From<&PuzzleJson> for EditablePuzzle {
    fn from(j: &PuzzleJson) -> Self {
        Self {
            width: j.width,
            height: j.height,
            starts: if j.starts.is_empty() {
                vec![[0, 0]]
            } else {
                j.starts.clone()
            },
            ends: if j.ends.is_empty() {
                vec![[j.width, j.height]]
            } else {
                j.ends.clone()
            },
            symmetry: j.symmetry,
            squares: j.squares.clone(),
            stars: j.stars.clone(),
            triangles: j.triangles.clone(),
            tetris: j.tetris.clone(),
            eliminations: j.eliminations.clone(),
            broken_edges: j.broken_edges.clone(),
            node_dots: j.node_dots.clone(),
            edge_dots: j.edge_dots.clone(),
            colored_node_dots: j
                .colored_node_dots
                .iter()
                .map(|d| (d.pos, d.color))
                .collect(),
            colored_edge_dots: j
                .colored_edge_dots
                .iter()
                .map(|d| (d.endpoints, d.color))
                .collect(),
            suns: j.sun_cells.clone(),
        }
    }
}

impl EditablePuzzle {
    fn to_json(&self) -> PuzzleJson {
        PuzzleJson::from(self)
    }

    fn from_json(j: &PuzzleJson) -> Self {
        EditablePuzzle::from(j)
    }

    fn cell_constraint(&self, cx: usize, cy: usize) -> Option<CellConstraint> {
        for sq in &self.squares {
            if sq.pos == [cx, cy] {
                return Some(CellConstraint::Square { color: sq.color });
            }
        }
        for st in &self.stars {
            if st.pos == [cx, cy] {
                return Some(CellConstraint::Star { color: st.color });
            }
        }
        for su in &self.suns {
            if su.pos == [cx, cy] {
                return Some(CellConstraint::Sun { color: su.color });
            }
        }
        for tr in &self.triangles {
            if tr.pos == [cx, cy] {
                return Some(CellConstraint::Triangle { count: tr.count });
            }
        }
        for te in &self.tetris {
            if te.pos == [cx, cy] {
                return Some(CellConstraint::Tetris {
                    shape: te.shape.clone(),
                    negative: te.negative,
                    can_rotate: te.can_rotate,
                });
            }
        }
        for el in &self.eliminations {
            if *el == [cx, cy] {
                return Some(CellConstraint::Elimination);
            }
        }
        None
    }

    fn has_broken_edge(&self, u: [usize; 2], v: [usize; 2]) -> bool {
        self.broken_edges
            .iter()
            .any(|e| (e[0] == u && e[1] == v) || (e[0] == v && e[1] == u))
    }

    fn has_edge_dot(&self, u: [usize; 2], v: [usize; 2]) -> bool {
        self.edge_dots
            .iter()
            .any(|e| (e[0] == u && e[1] == v) || (e[0] == v && e[1] == u))
    }

    fn remove_cell_constraint(&mut self, cx: usize, cy: usize) {
        self.squares.retain(|s| s.pos != [cx, cy]);
        self.stars.retain(|s| s.pos != [cx, cy]);
        self.suns.retain(|s| s.pos != [cx, cy]);
        self.triangles.retain(|t| t.pos != [cx, cy]);
        self.tetris.retain(|t| t.pos != [cx, cy]);
        self.eliminations.retain(|e| *e != [cx, cy]);
    }

    fn remove_broken_edge(&mut self, u: [usize; 2], v: [usize; 2]) {
        self.broken_edges
            .retain(|e| !((e[0] == u && e[1] == v) || (e[0] == v && e[1] == u)));
    }

    fn remove_edge_dot(&mut self, u: [usize; 2], v: [usize; 2]) {
        self.edge_dots
            .retain(|e| !((e[0] == u && e[1] == v) || (e[0] == v && e[1] == u)));
    }

    fn resize(&mut self, new_w: usize, new_h: usize) {
        self.width = new_w;
        self.height = new_h;

        for s in &mut self.starts {
            s[0] = s[0].min(new_w);
            s[1] = s[1].min(new_h);
        }
        for e in &mut self.ends {
            e[0] = e[0].min(new_w);
            e[1] = e[1].min(new_h);
        }

        self.squares
            .retain(|s| s.pos[0] < new_w && s.pos[1] < new_h);
        self.stars.retain(|s| s.pos[0] < new_w && s.pos[1] < new_h);
        self.suns.retain(|s| s.pos[0] < new_w && s.pos[1] < new_h);
        self.triangles
            .retain(|t| t.pos[0] < new_w && t.pos[1] < new_h);
        self.tetris.retain(|t| t.pos[0] < new_w && t.pos[1] < new_h);
        self.eliminations.retain(|e| e[0] < new_w && e[1] < new_h);
        self.node_dots.retain(|n| n[0] <= new_w && n[1] <= new_h);
        self.colored_node_dots
            .retain(|(n, _)| n[0] <= new_w && n[1] <= new_h);
        self.broken_edges.retain(|e| {
            e[0][0] <= new_w && e[0][1] <= new_h && e[1][0] <= new_w && e[1][1] <= new_h
        });
        self.edge_dots.retain(|e| {
            e[0][0] <= new_w && e[0][1] <= new_h && e[1][0] <= new_w && e[1][1] <= new_h
        });
        self.colored_edge_dots.retain(|(e, _)| {
            e[0][0] <= new_w && e[0][1] <= new_h && e[1][0] <= new_w && e[1][1] <= new_h
        });
    }

    fn cell_constraint_refs(&self, cx: usize, cy: usize) -> Vec<ConstraintRef> {
        let mut refs = vec![];
        for sq in &self.squares {
            if sq.pos == [cx, cy] {
                refs.push(ConstraintRef::Square {
                    pos: sq.pos,
                    color: sq.color,
                });
            }
        }
        for st in &self.stars {
            if st.pos == [cx, cy] {
                refs.push(ConstraintRef::Star {
                    pos: st.pos,
                    color: st.color,
                });
            }
        }
        for su in &self.suns {
            if su.pos == [cx, cy] {
                refs.push(ConstraintRef::Sun {
                    pos: su.pos,
                    color: su.color,
                });
            }
        }
        for tr in &self.triangles {
            if tr.pos == [cx, cy] {
                refs.push(ConstraintRef::Triangle {
                    pos: tr.pos,
                    count: tr.count,
                });
            }
        }
        for te in &self.tetris {
            if te.pos == [cx, cy] {
                refs.push(ConstraintRef::Tetris {
                    pos: te.pos,
                    negative: te.negative,
                });
            }
        }
        for el in &self.eliminations {
            if *el == [cx, cy] {
                refs.push(ConstraintRef::Elimination { pos: *el });
            }
        }
        refs
    }

    fn remove_constraint_ref(&mut self, cr: &ConstraintRef) {
        match cr {
            ConstraintRef::Square { pos, color } => {
                self.squares
                    .retain(|s| !(s.pos == *pos && s.color == *color));
            }
            ConstraintRef::Star { pos, color } => {
                self.stars.retain(|s| !(s.pos == *pos && s.color == *color));
            }
            ConstraintRef::Sun { pos, color } => {
                self.suns.retain(|s| !(s.pos == *pos && s.color == *color));
            }
            ConstraintRef::Triangle { pos, count } => {
                self.triangles
                    .retain(|t| !(t.pos == *pos && t.count == *count));
            }
            ConstraintRef::Tetris { pos, negative } => {
                self.tetris
                    .retain(|t| !(t.pos == *pos && t.negative == *negative));
            }
            ConstraintRef::Elimination { pos } => {
                self.eliminations.retain(|e| e != pos);
            }
        }
    }
}

#[derive(Debug, Clone)]
enum ConstraintRef {
    Square { pos: [usize; 2], color: u8 },
    Star { pos: [usize; 2], color: u8 },
    Sun { pos: [usize; 2], color: u8 },
    Triangle { pos: [usize; 2], count: u8 },
    Tetris { pos: [usize; 2], negative: bool },
    Elimination { pos: [usize; 2] },
}

impl ConstraintRef {
    fn label(&self) -> String {
        match self {
            ConstraintRef::Square { color, .. } => format!("Square ({})", color_name(*color)),
            ConstraintRef::Star { color, .. } => format!("Star ({})", color_name(*color)),
            ConstraintRef::Sun { color, .. } => format!("Sun ({})", color_name(*color)),
            ConstraintRef::Triangle { count, .. } => format!("Triangle {}", count),
            ConstraintRef::Tetris { negative, .. } => {
                if *negative {
                    "Tetris (-)".to_string()
                } else {
                    "Tetris".to_string()
                }
            }
            ConstraintRef::Elimination { .. } => "Elimination".to_string(),
        }
    }
}

fn tetris_preset(name: &str) -> Vec<[i8; 2]> {
    match name {
        "I" => vec![[0, 0], [0, 1], [0, 2], [0, 3]],
        "O" => vec![[0, 0], [1, 0], [0, 1], [1, 1]],
        "T" => vec![[0, 0], [-1, 0], [1, 0], [0, 1]],
        "L" => vec![[0, 0], [0, 1], [0, 2], [1, 2]],
        "J" => vec![[0, 0], [0, 1], [0, 2], [-1, 2]],
        "S" => vec![[0, 0], [1, 0], [-1, 1], [0, 1]],
        "Z" => vec![[0, 0], [-1, 0], [1, 1], [0, 1]],
        _ => vec![[0, 0]],
    }
}

fn ordered_solution_edges(g: &WitnessGraph, solution: &WitnessState) -> Vec<usize> {
    let mut stack = vec![(g.start, Vec::new(), vec![false; g.num_nodes()])];

    while let Some((node, path, mut visited)) = stack.pop() {
        if node == g.end {
            return path;
        }
        if visited[node] {
            continue;
        }
        visited[node] = true;

        let mut next = Vec::new();
        g.for_each_neighbor(node, |neighbor| {
            if visited[neighbor] {
                return;
            }
            let edge = g.edge_endpoints_to_idx(node, neighbor);
            if solution.used(edge) {
                next.push((neighbor, edge));
            }
        });

        for (neighbor, edge) in next.into_iter().rev() {
            let mut next_path = path.clone();
            next_path.push(edge);
            stack.push((neighbor, next_path, visited.clone()));
        }
    }

    Vec::new()
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct WitnessApp {
    puzzle: EditablePuzzle,
    current_tool: Tool,
    selected_color: u8,
    dot_color: u8,
    triangle_count: u8,
    solution: Option<WitnessState>,
    solution_edges: Vec<usize>,
    solution_animation_started: Option<Instant>,
    graph: Option<WitnessGraph>,
    solving: bool,
    solve_error: Option<String>,
    solve_report: Option<SolverReport>,
    show_solution: bool,
    hover_pos: Option<HitTarget>,
    last_file_path: Option<String>,
    tetris_editor_shape: Vec<[i8; 2]>,
    tetris_negative: bool,
    tetris_editor_can_rotate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitKind {
    Node,
    HEdge,
    VEdge,
    Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HitTarget {
    coords: [usize; 2],
    kind: HitKind,
}

impl HitTarget {
    fn edge_endpoints(self) -> Option<([usize; 2], [usize; 2])> {
        match self.kind {
            HitKind::HEdge => Some((self.coords, [self.coords[0] + 1, self.coords[1]])),
            HitKind::VEdge => Some((self.coords, [self.coords[0], self.coords[1] + 1])),
            _ => None,
        }
    }

    fn label(self) -> String {
        match self.kind {
            HitKind::Node => format!("Node ({}, {})", self.coords[0], self.coords[1]),
            HitKind::Cell => format!("Cell ({}, {})", self.coords[0], self.coords[1]),
            HitKind::HEdge | HitKind::VEdge => {
                let Some((u, v)) = self.edge_endpoints() else {
                    return "Edge".to_string();
                };
                format!("Edge ({}, {}) -> ({}, {})", u[0], u[1], v[0], v[1])
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BoardGeometry {
    panel: egui::Rect,
    board: egui::Rect,
    cell_size: f32,
    channel_width: f32,
    node_radius: f32,
    end_stub: f32,
}

impl BoardGeometry {
    fn new(canvas: egui::Rect, w: usize, h: usize) -> Option<Self> {
        if w == 0 || h == 0 {
            return None;
        }

        let padding = (canvas.width().min(canvas.height()) * 0.09 + 28.0).clamp(34.0, 92.0);
        let available_w = (canvas.width() - padding * 2.0).max(w as f32 * 18.0);
        let available_h = (canvas.height() - padding * 2.0).max(h as f32 * 18.0);
        let cell_size = (available_w / w as f32)
            .min(available_h / h as f32)
            .clamp(18.0, 104.0);
        let grid_w = cell_size * w as f32;
        let grid_h = cell_size * h as f32;
        let min = egui::pos2(
            canvas.center().x - grid_w * 0.5,
            canvas.center().y - grid_h * 0.5,
        );
        let board = egui::Rect::from_min_size(min, egui::vec2(grid_w, grid_h));
        let channel_width = (cell_size * 0.22).clamp(8.5, 24.0);
        let node_radius = channel_width * 0.52;
        let end_stub = (cell_size * 0.38).clamp(18.0, 42.0);
        let panel_pad = end_stub + channel_width * 1.45;
        let bounds = canvas.shrink(12.0);
        let panel = egui::Rect::from_min_max(
            egui::pos2(
                (board.min.x - panel_pad).max(bounds.min.x),
                (board.min.y - panel_pad).max(bounds.min.y),
            ),
            egui::pos2(
                (board.max.x + panel_pad).min(bounds.max.x),
                (board.max.y + panel_pad).min(bounds.max.y),
            ),
        );

        Some(Self {
            panel,
            board,
            cell_size,
            channel_width,
            node_radius,
            end_stub,
        })
    }

    fn node_pos(self, nx: usize, ny: usize) -> egui::Pos2 {
        egui::pos2(
            self.board.min.x + nx as f32 * self.cell_size,
            self.board.min.y + ny as f32 * self.cell_size,
        )
    }

    fn cell_rect(self, cx: usize, cy: usize) -> egui::Rect {
        egui::Rect::from_min_max(self.node_pos(cx, cy), self.node_pos(cx + 1, cy + 1))
    }
}

impl Default for WitnessApp {
    fn default() -> Self {
        Self {
            puzzle: EditablePuzzle::default(),
            current_tool: Tool::Start,
            selected_color: 1,
            dot_color: 0,
            triangle_count: 1,
            solution: None,
            solution_edges: Vec::new(),
            solution_animation_started: None,
            graph: None,
            solving: false,
            solve_error: None,
            solve_report: None,
            show_solution: true,
            hover_pos: None,
            last_file_path: None,
            tetris_editor_shape: vec![[0, 0]],
            tetris_negative: false,
            tetris_editor_can_rotate: true,
        }
    }
}

impl WitnessApp {
    pub fn new(puzzle: Option<EditablePuzzle>) -> Self {
        let mut app = Self::default();
        if let Some(p) = puzzle {
            app.puzzle = p;
        }
        app
    }

    fn solve_puzzle(&mut self) {
        self.solving = true;
        self.solve_error = None;
        self.solution = None;
        self.solution_edges.clear();
        self.solution_animation_started = None;
        self.solve_report = None;

        let json = self.puzzle.to_json();
        match WitnessGraph::from_json(json) {
            Ok(graph) => {
                let (sol, report) = solve_graph(
                    &graph,
                    SolverConfig {
                        parallel: true,
                        split_depth: 3,
                        auto_split: false,
                        pruner_profile: PrunerProfile::All,
                    },
                );
                if let Some(s) = sol {
                    self.solution_edges = ordered_solution_edges(&graph, &s);
                    self.solution_animation_started = Some(Instant::now());
                    self.solution = Some(s);
                    self.graph = Some(graph);
                } else {
                    self.solve_error = Some(format!(
                        "No solution found. Explored {} nodes.",
                        report.nodes
                    ));
                    self.graph = Some(graph);
                }
                self.solve_report = Some(report);
            }
            Err(e) => {
                self.solve_error = Some(format!("Invalid puzzle: {e}"));
            }
        }
        self.solving = false;
    }

    fn clear_solution(&mut self) {
        self.solution = None;
        self.solution_edges.clear();
        self.solution_animation_started = None;
        self.solve_error = None;
        self.solve_report = None;
    }

    fn visible_solution_edge_count(&self) -> usize {
        if self.solution_edges.is_empty() {
            return 0;
        }

        let Some(started) = self.solution_animation_started else {
            return self.solution_edges.len();
        };

        let step = Duration::from_millis(90);
        let visible = (started.elapsed().as_millis() / step.as_millis()) as usize + 1;
        visible.min(self.solution_edges.len())
    }

    fn load_file(&mut self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<PuzzleJson>(&text) {
                Ok(json) => {
                    self.puzzle = EditablePuzzle::from_json(&json);
                    self.clear_solution();
                    self.last_file_path = Some(path.to_string());
                }
                Err(e) => {
                    self.solve_error = Some(format!("JSON parse error: {e}"));
                }
            },
            Err(e) => {
                self.solve_error = Some(format!("Failed to read file: {e}"));
            }
        }
    }

    fn save_file(&mut self, path: &str) {
        let json = self.puzzle.to_json();
        match serde_json::to_string_pretty(&json) {
            Ok(text) => {
                if let Err(e) = std::fs::write(path, text) {
                    self.solve_error = Some(format!("Failed to write file: {e}"));
                } else {
                    self.last_file_path = Some(path.to_string());
                }
            }
            Err(e) => {
                self.solve_error = Some(format!("JSON serialize error: {e}"));
            }
        }
    }

    fn selected_tool_label(&self) -> String {
        match self.current_tool {
            Tool::Triangle => format!("Triangle {}", self.triangle_count),
            Tool::Tetris if self.tetris_negative => "Tetris (-)".to_string(),
            _ => self.current_tool.name().to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// eframe App
// ---------------------------------------------------------------------------

impl eframe::App for WitnessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        if self.solution_animation_started.is_some() {
            if self.visible_solution_edge_count() < self.solution_edges.len() {
                ctx.request_repaint_after(Duration::from_millis(16));
            } else {
                self.solution_animation_started = None;
            }
        }

        self.show_top_bar(ctx);
        self.show_left_sidebar(ctx);
        self.show_right_sidebar(ctx);
        self.show_canvas(ctx);
    }
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

impl WitnessApp {
    fn panel_frame(fill: egui::Color32) -> egui::Frame {
        egui::Frame::none()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0_f32, colors::SIDEBAR_STROKE))
    }

    fn show_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar")
            .resizable(false)
            .frame(Self::panel_frame(colors::TOP_BAR))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Open...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("JSON", &["json"])
                                .pick_file()
                            {
                                self.load_file(path.to_str().unwrap_or(""));
                            }
                            ui.close_menu();
                        }
                        if ui.button("Save As...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("JSON", &["json"])
                                .save_file()
                            {
                                self.save_file(path.to_str().unwrap_or(""));
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("New Puzzle").clicked() {
                            self.puzzle = EditablePuzzle::default();
                            self.clear_solution();
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    let solve_btn = egui::Button::new(egui::RichText::new("Solve").strong())
                        .fill(colors::PATH)
                        .stroke(egui::Stroke::new(1.0_f32, colors::BOARD_RIM));
                    if ui.add_sized([72.0, 28.0], solve_btn).clicked() {
                        self.solve_puzzle();
                    }
                    if ui
                        .add_sized([72.0, 28.0], egui::Button::new("Reset"))
                        .clicked()
                    {
                        self.clear_solution();
                    }
                    ui.toggle_value(&mut self.show_solution, "Show");
                    if self.solution.is_some()
                        && ui
                            .add_sized([72.0, 28.0], egui::Button::new("Replay"))
                            .clicked()
                    {
                        self.solution_animation_started = Some(Instant::now());
                        self.show_solution = true;
                    }

                    if self.solving {
                        ui.spinner();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let file_label = self
                            .last_file_path
                            .as_deref()
                            .and_then(|path| std::path::Path::new(path).file_name())
                            .and_then(|name| name.to_str())
                            .unwrap_or("Untitled");
                        ui.label(egui::RichText::new(file_label).color(colors::TEXT_MUTED));
                        ui.separator();
                        ui.label(format!("{}x{}", self.puzzle.width, self.puzzle.height));
                    });
                });
                ui.add_space(4.0);
            });
    }

    fn show_left_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools")
            .resizable(false)
            .default_width(168.0)
            .frame(Self::panel_frame(colors::SIDEBAR))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading("Tools");
                ui.add_space(6.0);

                self.tool_group(ui, "Path", &[Tool::Start, Tool::End]);
                self.tool_group(
                    ui,
                    "Cells",
                    &[
                        Tool::Square,
                        Tool::Star,
                        Tool::Sun,
                        Tool::Triangle,
                        Tool::Tetris,
                        Tool::Elimination,
                    ],
                );

                if self.current_tool.uses_cell_color() {
                    ui.add_space(6.0);
                    self.show_cell_color_swatches(ui);
                }

                if self.current_tool == Tool::Triangle {
                    ui.add_space(6.0);
                    self.show_triangle_control(ui);
                }

                if self.current_tool == Tool::Tetris {
                    ui.add_space(6.0);
                    self.show_tetris_editor(ui);
                }

                self.tool_group(
                    ui,
                    "Lines",
                    &[Tool::BrokenEdge, Tool::NodeDot, Tool::EdgeDot],
                );

                if self.current_tool.uses_dot_color() {
                    ui.add_space(6.0);
                    self.show_dot_color_swatches(ui);
                }

                self.tool_group(ui, "Edit", &[Tool::Eraser]);

                ui.add_space(10.0);
                egui::Frame::none()
                    .fill(colors::CONTROL_BG)
                    .rounding(6.0)
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Selected")
                                .small()
                                .color(colors::TEXT_MUTED),
                        );
                        ui.label(egui::RichText::new(self.selected_tool_label()).strong());
                    });
            });
    }

    fn tool_group(&mut self, ui: &mut egui::Ui, title: &str, tools: &[Tool]) {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(title)
                .small()
                .strong()
                .color(colors::TEXT_MUTED),
        );
        ui.add_space(3.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 5.0);
            for &tool in tools {
                self.tool_button(ui, tool);
            }
        });
    }

    fn tool_button(&mut self, ui: &mut egui::Ui, tool: Tool) {
        let active = self.current_tool == tool;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(36.0, 34.0), egui::Sense::click());
        let clicked = response.clicked();
        let hovered = response.hovered();
        response.on_hover_text(tool.name());

        let fill = if active {
            colors::CONTROL_ACTIVE
        } else if hovered {
            colors::SIDEBAR_STROKE
        } else {
            colors::CONTROL_BG
        };
        let stroke = if active {
            egui::Stroke::new(2.0_f32, colors::PATH)
        } else {
            egui::Stroke::new(1.0_f32, colors::SIDEBAR_STROKE)
        };

        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, 5.0, fill);
            ui.painter().rect_stroke(rect, 5.0, stroke);
            draw_tool_icon(ui.painter(), tool, rect.shrink(6.0), active);
        }

        if clicked {
            self.current_tool = tool;
        }
    }

    fn show_cell_color_swatches(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Color")
                .small()
                .color(colors::TEXT_MUTED),
        );
        ui.horizontal_wrapped(|ui| {
            for (color, c) in [
                (1, colors::SQUARE_BLACK),
                (2, colors::SQUARE_WHITE),
                (3, colors::RED),
                (4, colors::GREEN),
                (5, colors::ORANGE),
            ] {
                self.color_swatch(ui, color, c, color_name(color), true);
            }
        });
    }

    fn show_dot_color_swatches(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Dot Color")
                .small()
                .color(colors::TEXT_MUTED),
        );
        ui.horizontal_wrapped(|ui| {
            for (color, name) in [(0, "Black"), (1, "Blue"), (2, "Yellow"), (3, "Cyan")] {
                self.color_swatch(ui, color, dot_color32(color), name, false);
            }
        });
    }

    fn color_swatch(
        &mut self,
        ui: &mut egui::Ui,
        value: u8,
        color: egui::Color32,
        name: &str,
        cell_color: bool,
    ) {
        let active = if cell_color {
            self.selected_color == value
        } else {
            self.dot_color == value
        };
        let stroke = if active {
            egui::Stroke::new(2.5_f32, colors::PATH)
        } else {
            egui::Stroke::new(1.0_f32, colors::DOT_RING)
        };
        let response = ui
            .add_sized(
                [25.0, 25.0],
                egui::Button::new("").fill(color).stroke(stroke),
            )
            .on_hover_text(name);
        if response.clicked() {
            if cell_color {
                self.selected_color = value;
            } else {
                self.dot_color = value;
            }
        }
    }

    fn show_triangle_control(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Count")
                .small()
                .color(colors::TEXT_MUTED),
        );
        ui.horizontal(|ui| {
            for count in 1..=3 {
                ui.selectable_value(&mut self.triangle_count, count, count.to_string());
            }
        });
    }

    fn show_tetris_editor(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new("Shape")
                .small()
                .strong()
                .color(colors::TEXT_MUTED),
        );
        ui.horizontal_wrapped(|ui| {
            for (label, preset_name) in [
                ("1", "Single"),
                ("I", "I"),
                ("O", "O"),
                ("T", "T"),
                ("L", "L"),
                ("J", "J"),
                ("S", "S"),
                ("Z", "Z"),
            ] {
                if ui
                    .add_sized([24.0, 23.0], egui::Button::new(label))
                    .clicked()
                {
                    self.tetris_editor_shape = tetris_preset(preset_name);
                }
            }
        });

        ui.add_space(4.0);
        for gy in 0..4 {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                for gx in 0..4 {
                    let offset = [gx as i8 - 1, gy as i8];
                    let is_origin = offset == [0, 0];
                    let is_on = self.tetris_editor_shape.contains(&offset);
                    let fill = if is_on {
                        if self.tetris_negative {
                            colors::TETRIS_NEG
                        } else {
                            colors::TETRIS
                        }
                    } else {
                        colors::CONTROL_BG
                    };
                    let stroke = if is_origin {
                        egui::Stroke::new(1.5_f32, colors::TEXT)
                    } else {
                        egui::Stroke::new(0.75_f32, colors::SIDEBAR_STROKE)
                    };
                    if ui
                        .add_sized(
                            [20.0, 20.0],
                            egui::Button::new("").fill(fill).stroke(stroke),
                        )
                        .clicked()
                    {
                        if is_on {
                            self.tetris_editor_shape.retain(|o| *o != offset);
                        } else {
                            self.tetris_editor_shape.push(offset);
                        }
                    }
                }
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.toggle_value(&mut self.tetris_negative, "Negative");
            ui.checkbox(&mut self.tetris_editor_can_rotate, "Rotate");
        });
    }

    fn show_right_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .resizable(false)
            .default_width(230.0)
            .frame(Self::panel_frame(colors::SIDEBAR))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                self.show_inspector(ui);
                ui.separator();
                self.show_grid_settings(ui);
                ui.separator();
                self.show_solve_stats(ui);
            });
    }

    fn show_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inspector");
        ui.add_space(6.0);

        let Some(target) = self.hover_pos else {
            ui.label(egui::RichText::new("No target").color(colors::TEXT_MUTED));
            return;
        };

        ui.label(egui::RichText::new(target.label()).strong());
        ui.add_space(4.0);

        match target.kind {
            HitKind::Cell => {
                let [cx, cy] = target.coords;
                let constraints = self.puzzle.cell_constraint_refs(cx, cy);
                if constraints.is_empty() {
                    ui.label(egui::RichText::new("Empty").color(colors::TEXT_MUTED));
                } else {
                    for cr in constraints {
                        let label = cr.label();
                        let should_remove = ui
                            .horizontal(|ui| {
                                ui.label(label);
                                ui.button("x").clicked()
                            })
                            .inner;
                        if should_remove {
                            self.puzzle.remove_constraint_ref(&cr);
                            self.clear_solution();
                        }
                    }
                }
            }
            HitKind::Node => {
                let coords = target.coords;
                let is_start = self.puzzle.starts.contains(&coords);
                let is_end = self.puzzle.ends.contains(&coords);
                let is_dot = self.puzzle.node_dots.contains(&coords);
                let colored_dot = self
                    .puzzle
                    .colored_node_dots
                    .iter()
                    .find(|(n, _)| *n == coords)
                    .map(|(_, c)| *c);

                ui.horizontal(|ui| {
                    ui.label("Start");
                    if is_start {
                        ui.colored_label(colors::PATH, "socket");
                        if ui.small_button("Remove").clicked() {
                            self.puzzle.starts.retain(|n| *n != coords);
                            self.clear_solution();
                        }
                    } else if ui.small_button("Set").clicked() {
                        if self.puzzle.starts.is_empty() {
                            self.puzzle.starts.push(coords);
                        } else {
                            self.puzzle.starts[0] = coords;
                        }
                        self.clear_solution();
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("End");
                    if is_end {
                        ui.colored_label(colors::TEXT, "exit");
                        if ui.small_button("Remove").clicked() {
                            self.puzzle.ends.retain(|n| *n != coords);
                            self.clear_solution();
                        }
                    } else if ui.small_button("Set").clicked() {
                        if self.puzzle.ends.is_empty() {
                            self.puzzle.ends.push(coords);
                        } else {
                            self.puzzle.ends[0] = coords;
                        }
                        self.clear_solution();
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Dot");
                    if is_dot {
                        ui.colored_label(colors::DOT_RING, "black");
                        if ui.small_button("Remove").clicked() {
                            self.puzzle.node_dots.retain(|n| *n != coords);
                            self.clear_solution();
                        }
                    } else if let Some(c) = colored_dot {
                        ui.colored_label(dot_color32(c), "colored");
                        if ui.small_button("Remove").clicked() {
                            self.puzzle.colored_node_dots.retain(|(n, _)| *n != coords);
                            self.clear_solution();
                        }
                    } else if ui.small_button("Add").clicked() {
                        self.puzzle.node_dots.push(coords);
                        self.clear_solution();
                    }
                });
            }
            HitKind::HEdge | HitKind::VEdge => {
                let Some((u, v)) = target.edge_endpoints() else {
                    return;
                };
                let is_broken = self.puzzle.has_broken_edge(u, v);
                let is_dot = self.puzzle.has_edge_dot(u, v);
                let colored_dot = self
                    .puzzle
                    .colored_edge_dots
                    .iter()
                    .find(|(e, _)| (e[0] == u && e[1] == v) || (e[0] == v && e[1] == u))
                    .map(|(_, c)| *c);

                ui.horizontal(|ui| {
                    ui.label("Channel");
                    if is_broken {
                        ui.colored_label(colors::TEXT_MUTED, "gap");
                        if ui.small_button("Repair").clicked() {
                            self.puzzle.remove_broken_edge(u, v);
                            self.clear_solution();
                        }
                    } else if ui.small_button("Break").clicked() {
                        self.puzzle.broken_edges.push([u, v]);
                        self.clear_solution();
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Dot");
                    if is_dot {
                        ui.colored_label(colors::DOT_RING, "black");
                        if ui.small_button("Remove").clicked() {
                            self.puzzle.remove_edge_dot(u, v);
                            self.clear_solution();
                        }
                    } else if let Some(c) = colored_dot {
                        ui.colored_label(dot_color32(c), "colored");
                        if ui.small_button("Remove").clicked() {
                            self.puzzle.colored_edge_dots.retain(|(e, _)| {
                                !((e[0] == u && e[1] == v) || (e[0] == v && e[1] == u))
                            });
                            self.clear_solution();
                        }
                    } else if ui.small_button("Add").clicked() {
                        self.puzzle.edge_dots.push([u, v]);
                        self.clear_solution();
                    }
                });
            }
        }
    }

    fn show_grid_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Grid");
        ui.add_space(6.0);
        let mut width = self.puzzle.width;
        let mut height = self.puzzle.height;
        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::DragValue::new(&mut width).range(1..=16))
                .changed();
            ui.label("x");
            changed |= ui
                .add(egui::DragValue::new(&mut height).range(1..=16))
                .changed();
        });
        if changed {
            self.puzzle.resize(width, height);
            self.clear_solution();
        }

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Symmetry")
                .small()
                .strong()
                .color(colors::TEXT_MUTED),
        );
        let before = self.puzzle.symmetry;
        egui::ComboBox::from_id_salt("symmetry_combo")
            .selected_text(match self.puzzle.symmetry {
                None => "None",
                Some(SymmetryKind::MirrorX) => "Mirror X",
                Some(SymmetryKind::MirrorY) => "Mirror Y",
                Some(SymmetryKind::MirrorXY) => "Mirror XY",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.puzzle.symmetry, None, "None");
                ui.selectable_value(
                    &mut self.puzzle.symmetry,
                    Some(SymmetryKind::MirrorX),
                    "Mirror X",
                );
                ui.selectable_value(
                    &mut self.puzzle.symmetry,
                    Some(SymmetryKind::MirrorY),
                    "Mirror Y",
                );
                ui.selectable_value(
                    &mut self.puzzle.symmetry,
                    Some(SymmetryKind::MirrorXY),
                    "Mirror XY",
                );
            });
        if before != self.puzzle.symmetry {
            self.clear_solution();
        }
    }

    fn show_solve_stats(&self, ui: &mut egui::Ui) {
        ui.heading("Solve Stats");
        ui.add_space(6.0);
        if let Some(report) = &self.solve_report {
            let nodes_per_sec = if report.elapsed_secs > 0.0 {
                report.nodes as f64 / report.elapsed_secs
            } else {
                0.0
            };
            ui.label(format!("Time: {:.3}s", report.elapsed_secs));
            ui.label(format!("Nodes: {}", report.nodes));
            ui.label(format!("Pruned: {}", report.pruned));
            ui.label(format!("Rate: {:.0}/s", nodes_per_sec));
            ui.label(format!(
                "Mode: {}",
                if report.parallel {
                    "parallel"
                } else {
                    "sequential"
                }
            ));
            ui.label(format!("Split: {}", report.split_depth));
            ui.label(format!("Pruners: {}", report.pruner_profile));
            for hit in &report.pruner_hits {
                ui.label(format!("{}: {}", hit.name, hit.hits));
            }
        } else {
            ui.label(egui::RichText::new("No run").color(colors::TEXT_MUTED));
        }
    }

    fn show_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(colors::APP_BG))
            .show(ctx, |ui| {
                let available = ui.available_size();
                let status_h = 38.0;
                let canvas_h = (available.y - status_h - 8.0).max(160.0);
                let canvas_size = egui::vec2(available.x.max(240.0), canvas_h);
                let (response, painter) =
                    ui.allocate_painter(canvas_size, egui::Sense::click_and_drag());

                painter.rect_filled(response.rect, 0.0, colors::CANVAS_BG);
                let w = self.puzzle.width;
                let h = self.puzzle.height;

                if let Some(geometry) = BoardGeometry::new(response.rect, w, h) {
                    self.handle_canvas_input(ui, &response, geometry, w, h);
                    self.hover_pos = response
                        .hover_pos()
                        .and_then(|pos| self.hit_test(pos, geometry, w, h));
                    self.paint_board(&painter, geometry, w, h);
                    if let Some(target) = self.hover_pos {
                        self.paint_hover(&painter, geometry, target);
                    }
                } else {
                    self.hover_pos = None;
                    painter.text(
                        response.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Empty grid",
                        egui::FontId::proportional(18.0),
                        colors::TEXT,
                    );
                }

                ui.add_space(6.0);
                self.show_status_strip(ui);
            });
    }

    fn show_status_strip(&self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(colors::TOP_BAR)
            .stroke(egui::Stroke::new(1.0_f32, colors::SIDEBAR_STROKE))
            .rounding(6.0)
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Selected:").color(colors::TEXT_MUTED));
                    ui.label(egui::RichText::new(self.selected_tool_label()).strong());
                    if let Some(target) = self.hover_pos {
                        ui.separator();
                        ui.label(target.label());
                    }
                    if self.solution.is_some() && self.show_solution {
                        ui.separator();
                        ui.colored_label(colors::SUCCESS_TEXT, "Solved");
                    } else if let Some(err) = &self.solve_error {
                        ui.separator();
                        ui.colored_label(colors::ERROR_TEXT, err);
                    }
                });
            });
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

impl WitnessApp {
    fn paint_board(&self, painter: &egui::Painter, geometry: BoardGeometry, w: usize, h: usize) {
        painter.rect_filled(
            geometry.panel.translate(egui::vec2(0.0, 5.0)),
            18.0,
            colors::CHANNEL_SHADOW,
        );
        painter.rect_filled(geometry.panel, 18.0, colors::BOARD_BG);
        painter.rect_stroke(
            geometry.panel,
            18.0,
            egui::Stroke::new(2.0_f32, colors::BOARD_RIM),
        );

        self.paint_cell_symbols(painter, geometry, w, h);
        self.paint_base_channels(painter, geometry, w, h);
        self.paint_solution_channels(painter, geometry, w, h);
        self.paint_edge_marks(painter, geometry, w, h);
        self.paint_node_marks(painter, geometry, w, h);
    }

    fn paint_cell_symbols(
        &self,
        painter: &egui::Painter,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
    ) {
        for cy in 0..h {
            for cx in 0..w {
                let rect = geometry.cell_rect(cx, cy);
                let center = rect.center();
                let size = geometry.cell_size * 0.34;
                if let Some(constraint) = self.puzzle.cell_constraint(cx, cy) {
                    match constraint {
                        CellConstraint::Square { color } => {
                            draw_square_symbol(painter, center, size * 0.9, symbol_color32(color));
                        }
                        CellConstraint::Star { color } => {
                            draw_star_symbol(painter, center, size * 0.92, symbol_color32(color));
                        }
                        CellConstraint::Sun { color } => {
                            draw_sun_symbol(painter, center, size * 0.86, symbol_color32(color));
                        }
                        CellConstraint::Triangle { count } => {
                            draw_triangle_count(painter, center, size, count);
                        }
                        CellConstraint::Tetris {
                            shape,
                            negative,
                            can_rotate,
                        } => {
                            draw_tetris_symbol(
                                painter,
                                center,
                                geometry.cell_size * 0.62,
                                &shape,
                                negative,
                                can_rotate,
                            );
                        }
                        CellConstraint::Elimination => {
                            draw_elimination_symbol(painter, center, size * 0.9);
                        }
                        CellConstraint::None => {}
                    }
                }
            }
        }
    }

    fn paint_base_channels(
        &self,
        painter: &egui::Painter,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
    ) {
        for y in 0..=h {
            for x in 0..w {
                let p1 = geometry.node_pos(x, y);
                let p2 = geometry.node_pos(x + 1, y);
                let u = [x, y];
                let v = [x + 1, y];
                if self.puzzle.has_broken_edge(u, v) {
                    draw_channel_gap(painter, p1, p2, geometry.channel_width, colors::CHANNEL);
                } else {
                    draw_channel_line(painter, p1, p2, geometry.channel_width, colors::CHANNEL);
                }
            }
        }

        for y in 0..h {
            for x in 0..=w {
                let p1 = geometry.node_pos(x, y);
                let p2 = geometry.node_pos(x, y + 1);
                let u = [x, y];
                let v = [x, y + 1];
                if self.puzzle.has_broken_edge(u, v) {
                    draw_channel_gap(painter, p1, p2, geometry.channel_width, colors::CHANNEL);
                } else {
                    draw_channel_line(painter, p1, p2, geometry.channel_width, colors::CHANNEL);
                }
            }
        }

        for &end in &self.puzzle.ends {
            self.paint_end_exit(painter, geometry, end, false);
        }

        for ny in 0..=h {
            for nx in 0..=w {
                let pos = geometry.node_pos(nx, ny);
                painter.circle_filled(
                    pos + egui::vec2(0.0, 1.5),
                    geometry.node_radius + 1.0,
                    colors::CHANNEL_SHADOW,
                );
                painter.circle_filled(pos, geometry.node_radius, colors::CHANNEL);
                painter.circle_filled(
                    pos + egui::vec2(-geometry.node_radius * 0.22, -geometry.node_radius * 0.22),
                    geometry.node_radius * 0.24,
                    colors::CHANNEL_HILITE,
                );
            }
        }
    }

    fn paint_solution_channels(
        &self,
        painter: &egui::Painter,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
    ) {
        let visible_count = self.visible_solution_edge_count();

        for y in 0..=h {
            for x in 0..w {
                let Some(graph) = &self.graph else {
                    continue;
                };
                let ei = graph.h_edge_index(x, y);
                let p1 = geometry.node_pos(x, y);
                let p2 = geometry.node_pos(x + 1, y);
                if self.puzzle.has_broken_edge([x, y], [x + 1, y]) {
                    continue;
                }
                if self.is_path_edge(ei, visible_count) {
                    draw_solution_line(painter, p1, p2, geometry.channel_width);
                } else if self.is_mirror_edge(ei, visible_count) {
                    painter.line_segment(
                        [p1, p2],
                        egui::Stroke::new(geometry.channel_width * 0.55, colors::MIRROR_PATH),
                    );
                }
            }
        }

        for y in 0..h {
            for x in 0..=w {
                let Some(graph) = &self.graph else {
                    continue;
                };
                let ei = graph.v_edge_index(x, y);
                let p1 = geometry.node_pos(x, y);
                let p2 = geometry.node_pos(x, y + 1);
                if self.puzzle.has_broken_edge([x, y], [x, y + 1]) {
                    continue;
                }
                if self.is_path_edge(ei, visible_count) {
                    draw_solution_line(painter, p1, p2, geometry.channel_width);
                } else if self.is_mirror_edge(ei, visible_count) {
                    painter.line_segment(
                        [p1, p2],
                        egui::Stroke::new(geometry.channel_width * 0.55, colors::MIRROR_PATH),
                    );
                }
            }
        }

        if self.solution.is_some()
            && self.show_solution
            && visible_count >= self.solution_edges.len()
            && !self.solution_edges.is_empty()
            && let Some(&end) = self.puzzle.ends.first()
        {
            self.paint_end_exit(painter, geometry, end, true);
        }
    }

    fn paint_edge_marks(
        &self,
        painter: &egui::Painter,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
    ) {
        for y in 0..=h {
            for x in 0..w {
                let p1 = geometry.node_pos(x, y);
                let p2 = geometry.node_pos(x + 1, y);
                let u = [x, y];
                let v = [x + 1, y];
                let mid = p1 + (p2 - p1) * 0.5;
                if self.puzzle.has_edge_dot(u, v) {
                    draw_channel_dot(painter, mid, dot_color32(0), geometry.channel_width);
                }
                if let Some((_, c)) = self
                    .puzzle
                    .colored_edge_dots
                    .iter()
                    .find(|(e, _)| (e[0] == u && e[1] == v) || (e[0] == v && e[1] == u))
                {
                    draw_channel_dot(painter, mid, dot_color32(*c), geometry.channel_width);
                }
            }
        }

        for y in 0..h {
            for x in 0..=w {
                let p1 = geometry.node_pos(x, y);
                let p2 = geometry.node_pos(x, y + 1);
                let u = [x, y];
                let v = [x, y + 1];
                let mid = p1 + (p2 - p1) * 0.5;
                if self.puzzle.has_edge_dot(u, v) {
                    draw_channel_dot(painter, mid, dot_color32(0), geometry.channel_width);
                }
                if let Some((_, c)) = self
                    .puzzle
                    .colored_edge_dots
                    .iter()
                    .find(|(e, _)| (e[0] == u && e[1] == v) || (e[0] == v && e[1] == u))
                {
                    draw_channel_dot(painter, mid, dot_color32(*c), geometry.channel_width);
                }
            }
        }
    }

    fn paint_node_marks(
        &self,
        painter: &egui::Painter,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
    ) {
        let mirror_start_nodes: Vec<[usize; 2]> = self
            .puzzle
            .symmetry
            .map(|kind| {
                self.puzzle
                    .starts
                    .iter()
                    .map(|&[x, y]| match kind {
                        SymmetryKind::MirrorX => [w - x, y],
                        SymmetryKind::MirrorY => [x, h - y],
                        SymmetryKind::MirrorXY => [w - x, h - y],
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mirror_end_nodes: Vec<[usize; 2]> = self
            .puzzle
            .symmetry
            .map(|kind| {
                self.puzzle
                    .ends
                    .iter()
                    .map(|&[x, y]| match kind {
                        SymmetryKind::MirrorX => [w - x, y],
                        SymmetryKind::MirrorY => [x, h - y],
                        SymmetryKind::MirrorXY => [w - x, h - y],
                    })
                    .collect()
            })
            .unwrap_or_default();

        let visible_count = self.visible_solution_edge_count();
        let active_start = self.solution.is_some() && self.show_solution && visible_count > 0;
        let active_end = self.solution.is_some()
            && self.show_solution
            && visible_count >= self.solution_edges.len()
            && !self.solution_edges.is_empty();

        for ny in 0..=h {
            for nx in 0..=w {
                let node = [nx, ny];
                let pos = geometry.node_pos(nx, ny);
                if self.puzzle.node_dots.contains(&node) {
                    draw_channel_dot(painter, pos, dot_color32(0), geometry.channel_width * 1.2);
                }
                if let Some((_, c)) = self
                    .puzzle
                    .colored_node_dots
                    .iter()
                    .find(|(n, _)| *n == node)
                {
                    draw_channel_dot(painter, pos, dot_color32(*c), geometry.channel_width * 1.2);
                }

                if self.puzzle.ends.contains(&node) {
                    draw_end_node(painter, pos, geometry, active_end);
                }
                if self.puzzle.starts.contains(&node) {
                    draw_start_socket(painter, pos, geometry, active_start);
                }
                if mirror_start_nodes.contains(&node) {
                    painter.circle_stroke(
                        pos,
                        geometry.channel_width * 0.86,
                        egui::Stroke::new(2.0_f32, colors::MIRROR_PATH),
                    );
                }
                if mirror_end_nodes.contains(&node) {
                    painter.circle_stroke(
                        pos,
                        geometry.channel_width * 0.66,
                        egui::Stroke::new(2.0_f32, colors::MIRROR_PATH),
                    );
                }
            }
        }
    }

    fn paint_hover(&self, painter: &egui::Painter, geometry: BoardGeometry, target: HitTarget) {
        match target.kind {
            HitKind::Node => {
                let pos = geometry.node_pos(target.coords[0], target.coords[1]);
                painter.circle_stroke(
                    pos,
                    geometry.channel_width,
                    egui::Stroke::new(2.2_f32, colors::HOVER_RING),
                );
            }
            HitKind::HEdge => {
                let p1 = geometry.node_pos(target.coords[0], target.coords[1]);
                let p2 = geometry.node_pos(target.coords[0] + 1, target.coords[1]);
                painter.line_segment(
                    [p1, p2],
                    egui::Stroke::new(geometry.channel_width * 1.18, colors::HOVER_RING),
                );
            }
            HitKind::VEdge => {
                let p1 = geometry.node_pos(target.coords[0], target.coords[1]);
                let p2 = geometry.node_pos(target.coords[0], target.coords[1] + 1);
                painter.line_segment(
                    [p1, p2],
                    egui::Stroke::new(geometry.channel_width * 1.18, colors::HOVER_RING),
                );
            }
            HitKind::Cell => {
                let rect = geometry
                    .cell_rect(target.coords[0], target.coords[1])
                    .shrink(3.0);
                painter.rect_filled(rect, 5.0, colors::HOVER_FILL);
                painter.rect_stroke(rect, 5.0, egui::Stroke::new(2.0_f32, colors::HOVER_RING));
            }
        }
    }

    fn paint_end_exit(
        &self,
        painter: &egui::Painter,
        geometry: BoardGeometry,
        end: [usize; 2],
        active: bool,
    ) {
        let Some(dir) = exit_direction(end, self.puzzle.width, self.puzzle.height) else {
            return;
        };
        let p1 = geometry.node_pos(end[0], end[1]);
        let p2 = p1 + dir * geometry.end_stub;
        if active {
            draw_solution_line(painter, p1, p2, geometry.channel_width);
            painter.circle_filled(p2, geometry.channel_width * 0.48, colors::PATH);
            painter.circle_filled(p2, geometry.channel_width * 0.26, colors::PATH_CORE);
        } else {
            draw_channel_line(painter, p1, p2, geometry.channel_width, colors::CHANNEL);
            painter.circle_filled(p2, geometry.channel_width * 0.5, colors::CHANNEL);
        }
    }

    fn is_path_edge(&self, edge_idx: usize, visible_count: usize) -> bool {
        if !self.show_solution {
            return false;
        }
        let Some(solution) = self.solution.as_ref() else {
            return false;
        };
        if self.solution_edges.is_empty() {
            solution.used(edge_idx)
        } else {
            self.solution_edges
                .iter()
                .take(visible_count)
                .any(|&ei| ei == edge_idx)
        }
    }

    fn is_mirror_edge(&self, edge_idx: usize, visible_count: usize) -> bool {
        if !self.show_solution {
            return false;
        }
        let Some(graph) = self.graph.as_ref() else {
            return false;
        };
        if graph.symmetry.is_none() {
            return false;
        }
        graph.symmetric_edge(edge_idx).is_some_and(|mirror_edge| {
            mirror_edge != edge_idx
                && self.is_path_edge(mirror_edge, visible_count)
                && !self.is_path_edge(edge_idx, visible_count)
        })
    }
}

fn draw_channel_line(
    painter: &egui::Painter,
    p1: egui::Pos2,
    p2: egui::Pos2,
    width: f32,
    color: egui::Color32,
) {
    painter.line_segment(
        [p1 + egui::vec2(0.0, 1.5), p2 + egui::vec2(0.0, 1.5)],
        egui::Stroke::new(width + 1.0, colors::CHANNEL_SHADOW),
    );
    painter.line_segment([p1, p2], egui::Stroke::new(width, color));
}

fn draw_channel_gap(
    painter: &egui::Painter,
    p1: egui::Pos2,
    p2: egui::Pos2,
    width: f32,
    color: egui::Color32,
) {
    let mid = p1 + (p2 - p1) * 0.5;
    let dir = (p2 - p1).normalized();
    let gap = width * 0.95;
    draw_channel_line(painter, p1, mid - dir * gap, width, color);
    draw_channel_line(painter, mid + dir * gap, p2, width, color);
}

fn draw_solution_line(painter: &egui::Painter, p1: egui::Pos2, p2: egui::Pos2, width: f32) {
    painter.line_segment([p1, p2], egui::Stroke::new(width * 1.75, colors::PATH_GLOW));
    painter.line_segment([p1, p2], egui::Stroke::new(width * 0.92, colors::PATH));
    painter.line_segment([p1, p2], egui::Stroke::new(width * 0.38, colors::PATH_CORE));
    painter.circle_filled(p1, width * 0.46, colors::PATH);
    painter.circle_filled(p2, width * 0.46, colors::PATH);
}

fn draw_channel_dot(
    painter: &egui::Painter,
    center: egui::Pos2,
    color: egui::Color32,
    channel_width: f32,
) {
    let radius = (channel_width * 0.31).clamp(3.5, 8.0);
    painter.circle_filled(center, radius + 1.6, colors::DOT_RING);
    painter.circle_filled(center, radius, color);
}

fn draw_start_socket(
    painter: &egui::Painter,
    pos: egui::Pos2,
    geometry: BoardGeometry,
    active: bool,
) {
    let outer = geometry.channel_width * 0.92;
    if active {
        painter.circle_filled(pos, outer * 1.55, colors::PATH_GLOW);
        painter.circle_filled(pos, outer * 1.02, colors::PATH);
        painter.circle_filled(pos, outer * 0.46, colors::PATH_CORE);
    } else {
        painter.circle_filled(pos, outer * 1.05, colors::CHANNEL);
        painter.circle_filled(pos, outer * 0.58, colors::BOARD_BG);
        painter.circle_stroke(
            pos,
            outer * 0.59,
            egui::Stroke::new(1.4_f32, colors::CHANNEL_HILITE),
        );
    }
}

fn draw_end_node(painter: &egui::Painter, pos: egui::Pos2, geometry: BoardGeometry, active: bool) {
    if active {
        painter.circle_filled(pos, geometry.channel_width * 0.5, colors::PATH);
        painter.circle_filled(pos, geometry.channel_width * 0.25, colors::PATH_CORE);
    } else {
        painter.circle_stroke(
            pos,
            geometry.channel_width * 0.58,
            egui::Stroke::new(1.6_f32, colors::CHANNEL_HILITE),
        );
    }
}

fn draw_tool_icon(painter: &egui::Painter, tool: Tool, rect: egui::Rect, active: bool) {
    let center = rect.center();
    let size = rect.width().min(rect.height());
    let accent = if active { colors::PATH } else { colors::TEXT };
    let muted = if active {
        colors::PATH_CORE
    } else {
        colors::TEXT_MUTED
    };

    match tool {
        Tool::Start => {
            let r = size * 0.35;
            painter.circle_filled(center, r, colors::CHANNEL);
            painter.circle_stroke(center, r, egui::Stroke::new(2.0_f32, accent));
            painter.circle_filled(center, r * 0.42, muted);
        }
        Tool::End => {
            let stroke = egui::Stroke::new(3.0_f32, accent);
            let left = center + egui::vec2(-size * 0.32, 0.0);
            let right = center + egui::vec2(size * 0.26, 0.0);
            painter.line_segment([left, right], stroke);
            painter.circle_filled(right, size * 0.2, accent);
        }
        Tool::Square => draw_square_symbol(painter, center, size * 0.56, accent),
        Tool::Star => draw_star_symbol(painter, center, size * 0.35, accent),
        Tool::Sun => draw_sun_symbol(painter, center, size * 0.36, accent),
        Tool::Triangle => draw_filled_triangle(painter, center, size * 0.58, colors::TRIANGLE),
        Tool::Tetris => {
            const TOOL_TETRIS_SHAPE: [[i8; 2]; 4] = [[0, 0], [1, 0], [0, 1], [0, 2]];
            draw_tetris_symbol(
                painter,
                center,
                size * 0.68,
                &TOOL_TETRIS_SHAPE,
                false,
                false,
            );
        }
        Tool::Elimination => draw_elimination_symbol(painter, center, size * 0.6),
        Tool::BrokenEdge => {
            let stroke = egui::Stroke::new(3.0_f32, accent);
            let left_a = center + egui::vec2(-size * 0.36, -size * 0.16);
            let left_b = center + egui::vec2(-size * 0.08, -size * 0.03);
            let right_a = center + egui::vec2(size * 0.08, size * 0.03);
            let right_b = center + egui::vec2(size * 0.36, size * 0.16);
            painter.line_segment([left_a, left_b], stroke);
            painter.line_segment([right_a, right_b], stroke);
            painter.line_segment(
                [
                    center + egui::vec2(-size * 0.08, size * 0.24),
                    center + egui::vec2(size * 0.08, -size * 0.24),
                ],
                egui::Stroke::new(2.0_f32, colors::ERROR_TEXT),
            );
        }
        Tool::NodeDot => {
            painter.circle_filled(center, size * 0.3, colors::DOT_RING);
            painter.circle_filled(center, size * 0.22, accent);
        }
        Tool::EdgeDot => {
            let stroke = egui::Stroke::new(3.0_f32, accent);
            painter.line_segment(
                [
                    center + egui::vec2(-size * 0.38, 0.0),
                    center + egui::vec2(size * 0.38, 0.0),
                ],
                stroke,
            );
            painter.circle_filled(center, size * 0.17, colors::DOT_RING);
            painter.circle_filled(center, size * 0.12, colors::DOT_BLACK);
        }
        Tool::Eraser => {
            let tilt = size * 0.18;
            let half_w = size * 0.32;
            let half_h = size * 0.2;
            let points = vec![
                center + egui::vec2(-half_w + tilt, -half_h),
                center + egui::vec2(half_w + tilt, -half_h),
                center + egui::vec2(half_w - tilt, half_h),
                center + egui::vec2(-half_w - tilt, half_h),
            ];
            painter.add(egui::Shape::convex_polygon(
                points,
                muted,
                egui::Stroke::new(1.2_f32, accent),
            ));
            painter.line_segment(
                [
                    center + egui::vec2(size * 0.02, -half_h),
                    center + egui::vec2(-size * 0.12, half_h),
                ],
                egui::Stroke::new(1.2_f32, colors::CONTROL_BG),
            );
        }
    }
}

fn draw_square_symbol(
    painter: &egui::Painter,
    center: egui::Pos2,
    size: f32,
    color: egui::Color32,
) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(size, size));
    painter.rect_filled(rect, 2.0, color);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(
            1.3_f32,
            if color == colors::SQUARE_BLACK {
                colors::DOT_RING
            } else {
                colors::SYMBOL_STROKE
            },
        ),
    );
}

fn draw_star_symbol(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    color: egui::Color32,
) {
    for i in 0..5 {
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 5.0;
        let tip = center + egui::vec2(angle.cos(), angle.sin()) * radius;
        let left_angle = angle - 0.38;
        let right_angle = angle + 0.38;
        let left = center + egui::vec2(left_angle.cos(), left_angle.sin()) * radius * 0.42;
        let right = center + egui::vec2(right_angle.cos(), right_angle.sin()) * radius * 0.42;
        painter.add(egui::Shape::convex_polygon(
            vec![tip, left, center, right],
            color,
            egui::Stroke::new(0.8_f32, colors::SYMBOL_STROKE),
        ));
    }
    painter.circle_filled(center, radius * 0.26, color);
}

fn draw_sun_symbol(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: egui::Color32) {
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = egui::vec2(angle.cos(), angle.sin());
        painter.line_segment(
            [center + dir * radius * 0.45, center + dir * radius],
            egui::Stroke::new(radius * 0.14, color),
        );
    }
    painter.circle_filled(center, radius * 0.48, color);
    painter.circle_stroke(
        center,
        radius * 0.5,
        egui::Stroke::new(1.0_f32, colors::SYMBOL_STROKE),
    );
}

fn draw_triangle_count(painter: &egui::Painter, center: egui::Pos2, size: f32, count: u8) {
    let tri_size = size * 0.52;
    let positions: &[egui::Vec2] = match count {
        1 => &[egui::vec2(0.0, 0.0)],
        2 => &[
            egui::vec2(-tri_size * 0.55, 0.0),
            egui::vec2(tri_size * 0.55, 0.0),
        ],
        _ => &[
            egui::vec2(0.0, -tri_size * 0.45),
            egui::vec2(-tri_size * 0.58, tri_size * 0.42),
            egui::vec2(tri_size * 0.58, tri_size * 0.42),
        ],
    };
    for offset in positions {
        draw_filled_triangle(painter, center + *offset, tri_size, colors::TRIANGLE);
    }
}

fn draw_filled_triangle(
    painter: &egui::Painter,
    center: egui::Pos2,
    size: f32,
    color: egui::Color32,
) {
    let h = size * 0.9;
    let points = vec![
        center + egui::vec2(0.0, -h * 0.55),
        center + egui::vec2(-size * 0.5, h * 0.42),
        center + egui::vec2(size * 0.5, h * 0.42),
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::new(1.0_f32, colors::SYMBOL_STROKE),
    ));
}

fn draw_tetris_symbol(
    painter: &egui::Painter,
    center: egui::Pos2,
    max_size: f32,
    shape: &[[i8; 2]],
    negative: bool,
    can_rotate: bool,
) {
    if shape.is_empty() {
        return;
    }

    let min_dx = shape.iter().map(|o| o[0]).min().unwrap_or(0);
    let min_dy = shape.iter().map(|o| o[1]).min().unwrap_or(0);
    let max_dx = shape.iter().map(|o| o[0]).max().unwrap_or(0);
    let max_dy = shape.iter().map(|o| o[1]).max().unwrap_or(0);
    let shape_w = (max_dx - min_dx + 1) as f32;
    let shape_h = (max_dy - min_dy + 1) as f32;
    let block = max_size / shape_w.max(shape_h);
    let origin = egui::pos2(
        center.x - shape_w * block * 0.5,
        center.y - shape_h * block * 0.5,
    );
    let rot = if can_rotate {
        egui::emath::Rot2::from_angle(0.22)
    } else {
        egui::emath::Rot2::from_angle(0.0)
    };
    let fill = if negative {
        egui::Color32::TRANSPARENT
    } else {
        colors::TETRIS
    };
    let stroke_color = if negative {
        colors::TETRIS_NEG
    } else {
        colors::SYMBOL_STROKE
    };
    let stroke_width = if negative { 2.0_f32 } else { 1.0_f32 };

    for [dx, dy] in shape.iter().copied() {
        let bx = origin.x + ((dx - min_dx) as f32 + 0.5) * block;
        let by = origin.y + ((dy - min_dy) as f32 + 0.5) * block;
        let rel = egui::vec2(bx - center.x, by - center.y);
        let rotated = rot * rel;
        let block_center = egui::pos2(center.x + rotated.x, center.y + rotated.y);
        let half = block * 0.43;
        let corners = [
            egui::vec2(-half, -half),
            egui::vec2(half, -half),
            egui::vec2(half, half),
            egui::vec2(-half, half),
        ];
        let poly: Vec<egui::Pos2> = corners
            .iter()
            .map(|&corner| {
                let r = rot * corner;
                block_center + r
            })
            .collect();
        painter.add(egui::Shape::convex_polygon(
            poly,
            fill,
            egui::Stroke::new(stroke_width, stroke_color),
        ));
    }
}

fn draw_elimination_symbol(painter: &egui::Painter, center: egui::Pos2, size: f32) {
    let radius = size * 0.55;
    let points: Vec<egui::Pos2> = (0..6)
        .map(|i| {
            let angle = std::f32::consts::FRAC_PI_6 + i as f32 * std::f32::consts::TAU / 6.0;
            center + egui::vec2(angle.cos(), angle.sin()) * radius
        })
        .collect();
    painter.add(egui::Shape::convex_polygon(
        points,
        colors::ELIMINATION,
        egui::Stroke::new(1.0_f32, colors::SYMBOL_STROKE),
    ));
    let stroke = egui::Stroke::new(size * 0.13, colors::TEXT_DARK);
    painter.line_segment(
        [
            center + egui::vec2(-radius * 0.45, -radius * 0.45),
            center + egui::vec2(radius * 0.45, radius * 0.45),
        ],
        stroke,
    );
    painter.line_segment(
        [
            center + egui::vec2(-radius * 0.45, radius * 0.45),
            center + egui::vec2(radius * 0.45, -radius * 0.45),
        ],
        stroke,
    );
}

fn exit_direction(node: [usize; 2], w: usize, h: usize) -> Option<egui::Vec2> {
    if node[0] == 0 {
        Some(egui::vec2(-1.0, 0.0))
    } else if node[0] == w {
        Some(egui::vec2(1.0, 0.0))
    } else if node[1] == 0 {
        Some(egui::vec2(0.0, -1.0))
    } else if node[1] == h {
        Some(egui::vec2(0.0, 1.0))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Interaction helpers
// ---------------------------------------------------------------------------

impl WitnessApp {
    fn handle_canvas_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
    ) {
        let shift = ui.input(|i| i.modifiers.shift);
        if response.secondary_clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                self.handle_right_click(pointer_pos, geometry, w, h);
            }
        } else if response.clicked() || response.dragged() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                self.handle_click(pointer_pos, geometry, w, h, shift);
            }
        }
    }

    fn hit_test(
        &self,
        pos: egui::Pos2,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
    ) -> Option<HitTarget> {
        let local_x = pos.x - geometry.board.min.x;
        let local_y = pos.y - geometry.board.min.y;
        let edge_slop = (geometry.channel_width * 0.72).max(8.0);
        let node_slop = (geometry.channel_width * 0.92).max(12.0);

        let mut best_dist = f32::MAX;
        let mut best = None;
        for ny in 0..=h {
            for nx in 0..=w {
                let npx = nx as f32 * geometry.cell_size;
                let npy = ny as f32 * geometry.cell_size;
                let dx = local_x - npx;
                let dy = local_y - npy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < node_slop && dist < best_dist {
                    best_dist = dist;
                    best = Some(HitTarget {
                        coords: [nx, ny],
                        kind: HitKind::Node,
                    });
                }
            }
        }
        if best.is_some() {
            return best;
        }

        for y in 0..=h {
            for x in 0..w {
                let p1x = x as f32 * geometry.cell_size;
                let p1y = y as f32 * geometry.cell_size;
                let p2x = (x + 1) as f32 * geometry.cell_size;
                let dist = point_to_segment_dist(local_x, local_y, p1x, p1y, p2x, p1y);
                if dist < edge_slop {
                    return Some(HitTarget {
                        coords: [x, y],
                        kind: HitKind::HEdge,
                    });
                }
            }
        }

        for y in 0..h {
            for x in 0..=w {
                let p1x = x as f32 * geometry.cell_size;
                let p1y = y as f32 * geometry.cell_size;
                let p2y = (y + 1) as f32 * geometry.cell_size;
                let dist = point_to_segment_dist(local_x, local_y, p1x, p1y, p1x, p2y);
                if dist < edge_slop {
                    return Some(HitTarget {
                        coords: [x, y],
                        kind: HitKind::VEdge,
                    });
                }
            }
        }

        if local_x >= 0.0 && local_y >= 0.0 {
            let cx = (local_x / geometry.cell_size) as usize;
            let cy = (local_y / geometry.cell_size) as usize;
            if cx < w && cy < h {
                return Some(HitTarget {
                    coords: [cx, cy],
                    kind: HitKind::Cell,
                });
            }
        }

        None
    }

    fn handle_click(
        &mut self,
        pos: egui::Pos2,
        geometry: BoardGeometry,
        w: usize,
        h: usize,
        shift: bool,
    ) {
        let Some(target) = self.hit_test(pos, geometry, w, h) else {
            return;
        };

        match target.kind {
            HitKind::Node => self.handle_node_click(target.coords, shift),
            HitKind::HEdge | HitKind::VEdge => {
                if let Some((u, v)) = target.edge_endpoints() {
                    self.handle_edge_click(u, v);
                }
            }
            HitKind::Cell => self.handle_cell_click(target.coords),
        }

        self.clear_solution();
    }

    fn handle_node_click(&mut self, node: [usize; 2], shift: bool) {
        match self.current_tool {
            Tool::Start => {
                if shift {
                    if !self.puzzle.starts.contains(&node) {
                        self.puzzle.starts.push(node);
                    }
                } else if self.puzzle.starts.is_empty() {
                    self.puzzle.starts.push(node);
                } else {
                    self.puzzle.starts[0] = node;
                }
            }
            Tool::End => {
                if shift {
                    if !self.puzzle.ends.contains(&node) {
                        self.puzzle.ends.push(node);
                    }
                } else if self.puzzle.ends.is_empty() {
                    self.puzzle.ends.push(node);
                } else {
                    self.puzzle.ends[0] = node;
                }
            }
            Tool::NodeDot => {
                if self.dot_color == 0 {
                    if self.puzzle.node_dots.contains(&node) {
                        self.puzzle.node_dots.retain(|n| *n != node);
                    } else {
                        self.puzzle.node_dots.push(node);
                    }
                } else {
                    let existing = self
                        .puzzle
                        .colored_node_dots
                        .iter()
                        .position(|(n, c)| *n == node && *c == self.dot_color);
                    if let Some(idx) = existing {
                        self.puzzle.colored_node_dots.remove(idx);
                    } else {
                        self.puzzle.colored_node_dots.push((node, self.dot_color));
                    }
                }
            }
            Tool::Eraser => {
                self.puzzle.node_dots.retain(|n| *n != node);
                self.puzzle.colored_node_dots.retain(|(n, _)| *n != node);
                self.puzzle.starts.retain(|n| *n != node);
                self.puzzle.ends.retain(|n| *n != node);
            }
            _ => {}
        }
    }

    fn handle_edge_click(&mut self, u: [usize; 2], v: [usize; 2]) {
        match self.current_tool {
            Tool::BrokenEdge => {
                if self.puzzle.has_broken_edge(u, v) {
                    self.puzzle.remove_broken_edge(u, v);
                } else {
                    self.puzzle.broken_edges.push([u, v]);
                }
            }
            Tool::EdgeDot => {
                if self.dot_color == 0 {
                    if self.puzzle.has_edge_dot(u, v) {
                        self.puzzle.remove_edge_dot(u, v);
                    } else {
                        self.puzzle.edge_dots.push([u, v]);
                    }
                } else {
                    let existing = self.puzzle.colored_edge_dots.iter().position(|(e, c)| {
                        ((e[0] == u && e[1] == v) || (e[0] == v && e[1] == u))
                            && *c == self.dot_color
                    });
                    if let Some(idx) = existing {
                        self.puzzle.colored_edge_dots.remove(idx);
                    } else {
                        self.puzzle.colored_edge_dots.push(([u, v], self.dot_color));
                    }
                }
            }
            Tool::Eraser => {
                self.puzzle.remove_broken_edge(u, v);
                self.puzzle.remove_edge_dot(u, v);
                self.puzzle
                    .colored_edge_dots
                    .retain(|(e, _)| !((e[0] == u && e[1] == v) || (e[0] == v && e[1] == u)));
            }
            _ => {}
        }
    }

    fn handle_cell_click(&mut self, cell: [usize; 2]) {
        let [cx, cy] = cell;
        match self.current_tool {
            Tool::Square => {
                self.puzzle.remove_cell_constraint(cx, cy);
                self.puzzle.squares.push(SquareJson {
                    pos: [cx, cy],
                    color: self.selected_color,
                });
            }
            Tool::Star => {
                self.puzzle.remove_cell_constraint(cx, cy);
                self.puzzle.stars.push(StarJson {
                    pos: [cx, cy],
                    color: self.selected_color,
                });
            }
            Tool::Sun => {
                self.puzzle.remove_cell_constraint(cx, cy);
                self.puzzle.suns.push(SunJson {
                    pos: [cx, cy],
                    color: self.selected_color,
                });
            }
            Tool::Triangle => {
                self.puzzle.remove_cell_constraint(cx, cy);
                self.puzzle.triangles.push(TriangleJson {
                    pos: [cx, cy],
                    count: self.triangle_count,
                });
            }
            Tool::Tetris => {
                self.puzzle.remove_cell_constraint(cx, cy);
                self.puzzle.tetris.push(TetrisJson {
                    pos: [cx, cy],
                    shape: self.tetris_editor_shape.clone(),
                    negative: self.tetris_negative,
                    can_rotate: self.tetris_editor_can_rotate,
                });
            }
            Tool::Elimination => {
                self.puzzle.remove_cell_constraint(cx, cy);
                self.puzzle.eliminations.push([cx, cy]);
            }
            Tool::Eraser => {
                self.puzzle.remove_cell_constraint(cx, cy);
            }
            _ => {}
        }
    }

    fn handle_right_click(&mut self, pos: egui::Pos2, geometry: BoardGeometry, w: usize, h: usize) {
        let Some(target) = self.hit_test(pos, geometry, w, h) else {
            return;
        };

        match target.kind {
            HitKind::Node => {
                self.puzzle.node_dots.retain(|n| *n != target.coords);
                self.puzzle
                    .colored_node_dots
                    .retain(|(n, _)| *n != target.coords);
                self.puzzle.starts.retain(|n| *n != target.coords);
                self.puzzle.ends.retain(|n| *n != target.coords);
            }
            HitKind::HEdge | HitKind::VEdge => {
                if let Some((u, v)) = target.edge_endpoints() {
                    self.puzzle.remove_broken_edge(u, v);
                    self.puzzle.remove_edge_dot(u, v);
                    self.puzzle
                        .colored_edge_dots
                        .retain(|(e, _)| !((e[0] == u && e[1] == v) || (e[0] == v && e[1] == u)));
                }
            }
            HitKind::Cell => {
                self.puzzle
                    .remove_cell_constraint(target.coords[0], target.coords[1]);
            }
        }

        self.clear_solution();
    }
}

fn point_to_segment_dist(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    if dx == 0.0 && dy == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * dx + (py - y1) * dy) / (dx * dx + dy * dy);
    let t = t.clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run_gui(puzzle_path: Option<String>) {
    let app = if let Some(path) = puzzle_path {
        let mut app = WitnessApp::default();
        app.load_file(&path);
        app
    } else {
        WitnessApp::default()
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 820.0])
            .with_title("Witness Puzzle Solver"),
        ..Default::default()
    };

    eframe::run_native(
        "Witness Puzzle Solver",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_geometry() -> BoardGeometry {
        BoardGeometry::new(
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(640.0, 560.0)),
            4,
            4,
        )
        .expect("valid board geometry")
    }

    #[test]
    fn hit_test_distinguishes_nodes_edges_and_cells() {
        let app = WitnessApp::default();
        let geometry = test_geometry();

        assert_eq!(
            app.hit_test(geometry.node_pos(0, 0), geometry, 4, 4),
            Some(HitTarget {
                coords: [0, 0],
                kind: HitKind::Node,
            })
        );

        let horizontal_edge =
            geometry.node_pos(1, 2) + (geometry.node_pos(2, 2) - geometry.node_pos(1, 2)) * 0.5;
        assert_eq!(
            app.hit_test(horizontal_edge, geometry, 4, 4),
            Some(HitTarget {
                coords: [1, 2],
                kind: HitKind::HEdge,
            })
        );

        let vertical_edge =
            geometry.node_pos(3, 1) + (geometry.node_pos(3, 2) - geometry.node_pos(3, 1)) * 0.5;
        assert_eq!(
            app.hit_test(vertical_edge, geometry, 4, 4),
            Some(HitTarget {
                coords: [3, 1],
                kind: HitKind::VEdge,
            })
        );

        assert_eq!(
            app.hit_test(geometry.cell_rect(2, 3).center(), geometry, 4, 4),
            Some(HitTarget {
                coords: [2, 3],
                kind: HitKind::Cell,
            })
        );
    }

    #[test]
    fn cell_tools_place_configured_triangle_tetris_and_erase() {
        let mut app = WitnessApp::default();
        let geometry = test_geometry();
        let cell_center = geometry.cell_rect(1, 1).center();

        app.current_tool = Tool::Triangle;
        app.triangle_count = 3;
        app.handle_click(cell_center, geometry, 4, 4, false);
        assert_eq!(app.puzzle.triangles.len(), 1);
        assert_eq!(app.puzzle.triangles[0].pos, [1, 1]);
        assert_eq!(app.puzzle.triangles[0].count, 3);

        app.current_tool = Tool::Tetris;
        app.tetris_negative = true;
        app.tetris_editor_can_rotate = false;
        app.tetris_editor_shape = tetris_preset("L");
        app.handle_click(cell_center, geometry, 4, 4, false);
        assert!(app.puzzle.triangles.is_empty());
        assert_eq!(app.puzzle.tetris.len(), 1);
        assert_eq!(app.puzzle.tetris[0].pos, [1, 1]);
        assert!(app.puzzle.tetris[0].negative);
        assert!(!app.puzzle.tetris[0].can_rotate);
        assert_eq!(app.puzzle.tetris[0].shape, tetris_preset("L"));

        app.current_tool = Tool::Eraser;
        app.handle_click(cell_center, geometry, 4, 4, false);
        assert!(app.puzzle.tetris.is_empty());
    }

    #[test]
    fn node_and_edge_tools_place_colored_dots_and_broken_edges() {
        let mut app = WitnessApp::default();
        let geometry = test_geometry();
        let node_pos = geometry.node_pos(2, 2);
        let edge_pos =
            geometry.node_pos(0, 1) + (geometry.node_pos(1, 1) - geometry.node_pos(0, 1)) * 0.5;

        app.current_tool = Tool::NodeDot;
        app.dot_color = 2;
        app.handle_click(node_pos, geometry, 4, 4, false);
        assert!(app.puzzle.node_dots.is_empty());
        assert_eq!(app.puzzle.colored_node_dots, vec![([2, 2], 2)]);

        app.handle_right_click(node_pos, geometry, 4, 4);
        assert!(app.puzzle.colored_node_dots.is_empty());

        app.current_tool = Tool::EdgeDot;
        app.dot_color = 3;
        app.handle_click(edge_pos, geometry, 4, 4, false);
        assert!(app.puzzle.edge_dots.is_empty());
        assert_eq!(app.puzzle.colored_edge_dots, vec![([[0, 1], [1, 1]], 3)]);

        app.current_tool = Tool::BrokenEdge;
        app.handle_click(edge_pos, geometry, 4, 4, false);
        assert_eq!(app.puzzle.broken_edges, vec![[[0, 1], [1, 1]]]);
        app.handle_click(edge_pos, geometry, 4, 4, false);
        assert!(app.puzzle.broken_edges.is_empty());
    }

    #[test]
    fn editable_json_roundtrip_preserves_gui_supported_constraints() {
        let json = PuzzleJson {
            width: 3,
            height: 2,
            starts: vec![[0, 0]],
            ends: vec![[3, 2]],
            node_dots: vec![[1, 1]],
            edge_dots: vec![[[0, 1], [1, 1]]],
            broken_edges: vec![[[2, 0], [3, 0]]],
            squares: vec![SquareJson {
                pos: [0, 0],
                color: 1,
            }],
            stars: vec![StarJson {
                pos: [1, 0],
                color: 3,
            }],
            triangles: vec![TriangleJson {
                pos: [2, 0],
                count: 2,
            }],
            tetris: vec![TetrisJson {
                pos: [0, 1],
                shape: tetris_preset("T"),
                negative: true,
                can_rotate: false,
            }],
            sun_cells: vec![SunJson {
                pos: [1, 1],
                color: 4,
            }],
            eliminations: vec![[2, 1]],
            colored_node_dots: vec![ColoredDotJson {
                pos: [2, 2],
                color: 2,
            }],
            colored_edge_dots: vec![ColoredEdgeDotJson {
                endpoints: [[1, 2], [2, 2]],
                color: 3,
            }],
            symmetry: Some(SymmetryKind::MirrorX),
        };

        let editable = EditablePuzzle::from_json(&json);
        let roundtrip = editable.to_json();
        assert_eq!(
            serde_json::to_value(&roundtrip).expect("roundtrip serializes"),
            serde_json::to_value(&json).expect("source serializes")
        );
    }

    #[test]
    fn resize_clamps_endpoints_and_prunes_out_of_bounds_constraints() {
        let mut puzzle = EditablePuzzle {
            width: 4,
            height: 4,
            starts: vec![[4, 4]],
            ends: vec![[3, 3]],
            squares: vec![
                SquareJson {
                    pos: [0, 0],
                    color: 1,
                },
                SquareJson {
                    pos: [3, 3],
                    color: 2,
                },
            ],
            triangles: vec![TriangleJson {
                pos: [2, 1],
                count: 1,
            }],
            broken_edges: vec![[[0, 0], [1, 0]], [[3, 3], [4, 3]]],
            node_dots: vec![[1, 1], [4, 4]],
            colored_edge_dots: vec![([[1, 1], [1, 2]], 2), ([[4, 3], [4, 4]], 3)],
            ..EditablePuzzle::default()
        };

        puzzle.resize(2, 2);

        assert_eq!(puzzle.starts, vec![[2, 2]]);
        assert_eq!(puzzle.ends, vec![[2, 2]]);
        assert_eq!(puzzle.squares.len(), 1);
        assert_eq!(puzzle.squares[0].pos, [0, 0]);
        assert!(puzzle.triangles.is_empty());
        assert_eq!(puzzle.broken_edges, vec![[[0, 0], [1, 0]]]);
        assert_eq!(puzzle.node_dots, vec![[1, 1]]);
        assert_eq!(puzzle.colored_edge_dots, vec![([[1, 1], [1, 2]], 2)]);
    }
}
