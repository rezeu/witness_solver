use std::{fs};
pub struct Graph {
    pub width: usize,
    pub height: usize,
    pub start: usize,
    pub end: usize,
}
impl Graph {
    pub fn from_file(filename: &str) -> Result<Self, &'static str> {
        let text = fs::read_to_string(filename).map_err(|_| "failed to read file")?;

        let mut w = None;
        let mut h = None;
        let mut start = None;
        let mut end = None;

        for line in text.lines() {
            let parts: Vec<_> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                // 第一行：w h
                _ if w.is_none() => {
                    if parts.len() != 2 {
                        return Err("invalid size line");
                    }
                    w = Some(parts[0].parse().map_err(|_| "bad w")?);
                    h = Some(parts[1].parse().map_err(|_| "bad h")?);
                }

                "start" => {
                    let x: usize = parts[1].parse().map_err(|_| "bad start x")?;
                    let y: usize = parts[2].parse().map_err(|_| "bad start y")?;
                    let ww = w.ok_or("w not set")?;
                    start = Some(y * (ww + 1) + x);
                }

                "end" => {
                    let x: usize = parts[1].parse().map_err(|_| "bad end x")?;
                    let y: usize = parts[2].parse().map_err(|_| "bad end y")?;
                    let ww = w.ok_or("w not set")?;
                    end = Some(y * (ww + 1) + x);
                }

                _ => {}
            }
        }

        Ok(Graph {
            width: w.ok_or("missing w")?,
            height: h.ok_or("missing h")?,
            start: start.ok_or("missing start")?,
            end: end.ok_or("missing end")?,
        })
    }
    pub fn node_idx_to_xy(&self, ni: usize) -> (usize, usize) {
        (ni % (self.width + 1), ni / (self.width + 1))
    }
    pub fn node_xy_to_idx(&self, x: usize, y: usize) -> usize {
        y * (self.width + 1) + x
    }
    pub fn edge_idx_to_endpoints(&self, ei: usize) -> (usize, usize) {
        //交错编码
        let real_idx = ei >> 1;
        if ei % 2 == 0 {
            //水平
            let y = real_idx / self.width;
            let x = real_idx % self.width;
            let u = self.node_xy_to_idx(x, y);
            let v = self.node_xy_to_idx(x + 1, y);
            (u, v)
        } else {
            let y = real_idx / (self.width+1);
            let x = real_idx % (self.width+1);
            let u = self.node_xy_to_idx(x, y);
            let v = self.node_xy_to_idx(x, y + 1);
            (u, v)
        }
    }
    pub fn edge_endpoints_to_idx(&self, u: usize, v: usize) -> usize {
        let (ux, uy) = self.node_idx_to_xy(u);
        let (vx, vy) = self.node_idx_to_xy(v);
        // let x = usize::min(ux, vx);
        // let y = usize::min(uy, vy);
        // let real_idx = y * self.width + x;
        // if uy == vy {
        //     //水平边
        //     real_idx << 1
        // } else {
        //     //垂直边
        //     (real_idx << 1) | 1
        // }
        if uy == vy {
            //水平边
            let y = uy;
            let x = usize::min(ux, vx);
            let real_idx = y * self.width + x;
            real_idx << 1
        } else {
            //垂直边
            let x = ux;
            let y = usize::min(uy, vy);
            let real_idx = y * (self.width + 1) + x;
            (real_idx << 1) | 1
        }
    }
    pub fn adj_nodes(&self, u: usize) -> Vec<usize> {
        let (ux, uy) = self.node_idx_to_xy(u);
        let mut neighbors = Vec::new();

        // 左
        if ux > 0 {
            neighbors.push(self.node_xy_to_idx(ux - 1, uy));
        }
        // 右
        if ux < self.width {
            neighbors.push(self.node_xy_to_idx(ux + 1, uy));
        }
        // 上
        if uy > 0 {
            neighbors.push(self.node_xy_to_idx(ux, uy - 1));
        }
        // 下
        if uy < self.height {
            neighbors.push(self.node_xy_to_idx(ux, uy + 1));
        }

        neighbors
    }
    pub fn has_h_edge(&self, x: usize, y: usize) -> bool {
        x < self.width && y <= self.height
    }
    pub fn has_v_edge(&self, x: usize, y: usize) -> bool {
        x <= self.width && y < self.height
    }
    pub fn h_edge_index(&self, x: usize, y: usize) -> usize {
        2 * (y * self.width + x)
    }
    pub fn v_edge_index(&self, x: usize, y: usize) -> usize {
        2 * (y * (self.width+1) + x) + 1
    }
    // pub fn cell_rule(&self, x: usize, y: usize) -> Option<&CellRule> {
    //     None
    // }
}

