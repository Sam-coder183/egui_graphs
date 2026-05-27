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

// Cardinality analysis removed in logmark-lite.

pub fn process_wikilinks_for_preview(content: &str) -> String {
    let re = regex::Regex::new(r"\[\[(.*?)(?:\|(.*?))?\]\]").unwrap();
    re.replace_all(content, |caps: &regex::Captures| {
        let target = &caps[1];
        let label = caps.get(2).map(|m| m.as_str()).unwrap_or(target);
        // Use a custom scheme that we might be able to catch, or just a placeholder
        format!("[{}]({}{})", label, "node://", target) 
    }).to_string()
}
