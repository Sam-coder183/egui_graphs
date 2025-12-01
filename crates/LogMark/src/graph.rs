use egui::{Color32, Pos2, Shape, Stroke, Vec2, FontId, FontFamily};
use egui_graphs::{DisplayNode, DisplayEdge, DrawContext, NodeProps, EdgeProps, Node};
use petgraph::Directed;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LogNodeData {
    pub label: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct LogNode {
    pub pos: Pos2,
    pub label: String,
    pub selected: bool,
    pub dragged: bool,
    pub hovered: bool,
    pub radius: f32,
}

impl From<NodeProps<LogNodeData>> for LogNode {
    fn from(node_props: NodeProps<LogNodeData>) -> Self {
        Self {
            pos: node_props.location(),
            label: node_props.payload.label.clone(),
            selected: node_props.selected,
            dragged: node_props.dragged,
            hovered: node_props.hovered,
            radius: 30.0,
        }
    }
}

impl DisplayNode<LogNodeData, (), Directed, u32> for LogNode {
    fn is_inside(&self, pos: Pos2) -> bool {
        let dir = pos - self.pos;
        dir.length() <= self.radius
    }

    fn closest_boundary_point(&self, dir: Vec2) -> Pos2 {
        self.pos + dir.normalized() * self.radius
    }

    fn shapes(&mut self, ctx: &DrawContext) -> Vec<Shape> {
        let mut shapes = Vec::new();
        let screen_pos = ctx.meta.canvas_to_screen_pos(self.pos);
        let screen_radius = ctx.meta.canvas_to_screen_size(self.radius);

        let color = if self.selected {
            Color32::from_rgb(100, 200, 255)
        } else if self.hovered {
            Color32::from_rgb(150, 150, 200)
        } else {
            Color32::from_rgb(100, 150, 200)
        };

        let stroke = if self.selected {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            Stroke::new(1.0, Color32::GRAY)
        };

        shapes.push(egui::epaint::CircleShape {
            center: screen_pos,
            radius: screen_radius,
            fill: color,
            stroke,
        }.into());

        let font_size = (screen_radius * 0.4).max(8.0).min(16.0);
        let galley = ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                self.label.clone(),
                FontId::new(font_size, FontFamily::Proportional),
                Color32::WHITE,
            )
        });

        let text_pos = Pos2::new(
            screen_pos.x - galley.size().x / 2.0,
            screen_pos.y - galley.size().y / 2.0,
        );

        shapes.push(egui::epaint::TextShape::new(text_pos, galley, Color32::WHITE).into());
        shapes
    }

    fn update(&mut self, state: &NodeProps<LogNodeData>) {
        self.pos = state.location();
        self.selected = state.selected;
        self.dragged = state.dragged;
        self.hovered = state.hovered;
        self.label = state.payload.label.clone();
    }
}

#[derive(Clone, Debug)]
pub struct LogEdge {
    pub selected: bool,
}

impl From<EdgeProps<()>> for LogEdge {
    fn from(edge_props: EdgeProps<()>) -> Self {
        Self {
            selected: edge_props.selected,
        }
    }
}

impl DisplayEdge<LogNodeData, (), Directed, u32, LogNode> for LogEdge {
    fn is_inside(
        &self,
        start: &Node<LogNodeData, (), Directed, u32, LogNode>,
        end: &Node<LogNodeData, (), Directed, u32, LogNode>,
        pos: Pos2,
    ) -> bool {
        let start_pos = start.location();
        let end_pos = end.location();
        let radius = 5.0;
        let line_vec = end_pos - start_pos;
        let point_vec = pos - start_pos;
        let line_len = line_vec.length();
        if line_len < 0.001 {
            return false;
        }
        let proj = point_vec.dot(line_vec) / line_len;
        if proj < 0.0 || proj > line_len {
            return false;
        }
        let closest = start_pos + line_vec.normalized() * proj;
        (pos - closest).length() <= radius
    }

    fn shapes(
        &mut self,
        start: &Node<LogNodeData, (), Directed, u32, LogNode>,
        end: &Node<LogNodeData, (), Directed, u32, LogNode>,
        ctx: &DrawContext,
    ) -> Vec<Shape> {
        let start_pos = start.location();
        let end_pos = end.location();

        let dir = (end_pos - start_pos).normalized();
        let start_boundary = start.display().closest_boundary_point(dir);
        let end_boundary = end.display().closest_boundary_point(-dir);
        
        let screen_start = ctx.meta.canvas_to_screen_pos(start_boundary);
        let screen_end = ctx.meta.canvas_to_screen_pos(end_boundary);

        let color = if self.selected {
            Color32::from_rgb(255, 200, 100)
        } else {
            Color32::from_rgb(128, 128, 128)
        };
        let stroke = Stroke::new(2.0, color);

        let mut shapes = Vec::new();
        shapes.push(egui::epaint::Shape::line_segment([screen_start, screen_end], stroke));

        // Arrow head
        let arrow_size = 10.0;
        let perp = Vec2::new(-dir.y, dir.x);
        let tip = screen_end - dir * arrow_size;
        let left = tip + perp * arrow_size * 0.5;
        let right = tip - perp * arrow_size * 0.5;

        shapes.push(egui::epaint::Shape::convex_polygon(
            vec![screen_end, left, right],
            color,
            Stroke::NONE,
        ));

        shapes
    }

    fn update(&mut self, state: &EdgeProps<()>) {
        self.selected = state.selected;
    }
}
