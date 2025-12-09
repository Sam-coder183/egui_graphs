use egui::Pos2;
use petgraph::Direction;
use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};
use egui_graphs::Graph;
use petgraph::Directed;

/// Helper function for 3D to 2D projection with all rotation axes
pub fn project_3d_to_2d(
    pos: Pos2,
    pivot: Pos2,
    z: f32,
    rotation_x: f32,
    rotation_y: f32,
    rotation_z: f32,
    perspective_strength: f32,
) -> Pos2 {
    let rx = rotation_x.to_radians();
    let ry = rotation_y.to_radians();
    let rz = rotation_z.to_radians();

    let x0 = pos.x - pivot.x;
    let y0 = pos.y - pivot.y;
    let z0 = z;
    
    // Apply rotation around X axis
    let y1 = y0 * rx.cos() - z0 * rx.sin();
    let z1 = y0 * rx.sin() + z0 * rx.cos();
    
    // Apply rotation around Y axis
    let x2 = x0 * ry.cos() + z1 * ry.sin();
    let z2 = -x0 * ry.sin() + z1 * ry.cos();
    
    // Apply rotation around Z axis
    let x3 = x2 * rz.cos() - y1 * rz.sin();
    let y2 = x2 * rz.sin() + y1 * rz.cos();
    
    // Perspective projection - depth affects scale
    let denom = 1.0 + z2 * perspective_strength;
    let scale = if denom.abs() < 0.000_1 { 1.0 } else { 1.0 / denom };
    let projected_local = Pos2::new(x3 * scale, y2 * scale);
    pivot + projected_local.to_vec2()
}

/// Calculate the cardinality of an edge based on the in/out degree of connected nodes
/// Returns a string like "1:1", "1:N", or "N:N"
pub fn calculate_edge_cardinality(
    graph: &Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>,
    source: petgraph::stable_graph::NodeIndex,
    target: petgraph::stable_graph::NodeIndex
) -> String {
    // Count outgoing edges from source to determine the "one" or "many" on source side
    let source_out_degree = graph.g().edges_directed(source, Direction::Outgoing).count();
    
    // Count incoming edges to target to determine the "one" or "many" on target side
    let target_in_degree = graph.g().edges_directed(target, Direction::Incoming).count();
    
    let source_card = if source_out_degree <= 1 { "1" } else { "N" };
    let target_card = if target_in_degree <= 1 { "1" } else { "N" };
    
    format!("{}:{}", source_card, target_card)
}
