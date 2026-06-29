use crate::witness::types::{EdgeId, NodeId};

#[inline]
pub fn node_xy_to_idx(width: usize, x: usize, y: usize) -> NodeId {
    y * (width + 1) + x
}

#[inline]
pub fn node_idx_to_xy(width: usize, node: NodeId) -> (usize, usize) {
    (node % (width + 1), node / (width + 1))
}

#[inline]
pub fn num_edge_slots(width: usize, height: usize) -> usize {
    2 * (height + 1) * (width + 1)
}

#[inline]
pub fn h_edge_index(width: usize, x: usize, y: usize) -> EdgeId {
    2 * (y * width + x)
}

#[inline]
pub fn v_edge_index(width: usize, x: usize, y: usize) -> EdgeId {
    2 * (y * (width + 1) + x) + 1
}

#[inline]
pub fn edge_endpoints_to_idx(width: usize, u: NodeId, v: NodeId) -> EdgeId {
    let ux = u % (width + 1);
    let uy = u / (width + 1);
    let vx = v % (width + 1);
    let vy = v / (width + 1);
    if uy == vy {
        let x = usize::min(ux, vx);
        h_edge_index(width, x, uy)
    } else {
        let x = ux;
        let y = usize::min(uy, vy);
        v_edge_index(width, x, y)
    }
}

#[inline]
pub fn edge_idx_to_endpoints(width: usize, edge: EdgeId) -> (NodeId, NodeId) {
    let real = edge >> 1;
    if edge & 1 == 0 {
        let y = real / width;
        let x = real % width;
        let u = node_xy_to_idx(width, x, y);
        let v = node_xy_to_idx(width, x + 1, y);
        (u, v)
    } else {
        let y = real / (width + 1);
        let x = real % (width + 1);
        let u = node_xy_to_idx(width, x, y);
        let v = node_xy_to_idx(width, x, y + 1);
        (u, v)
    }
}
