use egui::{Color32, Pos2, Shape, Stroke, Vec2, FontId, FontFamily};
use egui_graphs::{DisplayNode, DisplayEdge, DrawContext, NodeProps, EdgeProps, Node};
use petgraph::Directed;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LogNodeData {
    pub label: String,
    pub content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct LogEdgeData {
    pub label: Option<String>,
    pub cardinality: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LogNode {
    pub pos: Pos2,
    pub label: String,
    pub selected: bool,
    pub dragged: bool,
    pub hovered: bool,
    pub radius: f32,
    pub payload: LogNodeData,
}

impl LogNode {
    pub fn payload(&self) -> &LogNodeData {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut LogNodeData {
        &mut self.payload
    }
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
            payload: node_props.payload,
        }
    }
}

impl DisplayNode<LogNodeData, LogEdgeData, Directed, u32> for LogNode {
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

        // Scale text with zoom, but keep it legible
        // Increased base scale and minimum size for better readability
        let font_size = (screen_radius * 0.6).max(14.0).min(60.0);
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
        self.payload = state.payload.clone();
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LogEdge {
    pub selected: bool,
    pub label: Option<String>,
    pub cardinality: Option<String>,
}

impl From<EdgeProps<LogEdgeData>> for LogEdge {
    fn from(edge_props: EdgeProps<LogEdgeData>) -> Self {
        Self {
            selected: edge_props.selected,
            label: edge_props.payload.label.clone(),
            cardinality: edge_props.payload.cardinality.clone(),
        }
    }
}

impl DisplayEdge<LogNodeData, LogEdgeData, Directed, u32, LogNode> for LogEdge {
    fn is_inside(
        &self,
        start: &Node<LogNodeData, LogEdgeData, Directed, u32, LogNode>,
        end: &Node<LogNodeData, LogEdgeData, Directed, u32, LogNode>,
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
        start: &Node<LogNodeData, LogEdgeData, Directed, u32, LogNode>,
        end: &Node<LogNodeData, LogEdgeData, Directed, u32, LogNode>,
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
        let screen_dir = (screen_end - screen_start).normalized();
        let arrow_size = 10.0;
        let screen_perp = Vec2::new(-screen_dir.y, screen_dir.x);
        let tip = screen_end - screen_dir * arrow_size;
        let left = tip + screen_perp * arrow_size * 0.5;
        let right = tip - screen_perp * arrow_size * 0.5;

        shapes.push(egui::epaint::Shape::convex_polygon(
            vec![screen_end, left, right],
            color,
            Stroke::NONE,
        ));

        // Read settings
        let (base_font_size_label, base_font_size_card) = {
            if let Ok(settings) = crate::GRAPH_SETTINGS.read() {
                (settings.font_size_edge_label, settings.font_size_cardinality)
            } else {
                (14.0, 12.0)
            }
        };

        // Label
        if let Some(text) = &self.label {
            let mid = screen_start + (screen_end - screen_start) * 0.5;
            
            // Dynamic font size based on zoom
            let font_size = ctx.meta.canvas_to_screen_size(base_font_size_label).max(12.0).min(40.0);
            
            // Calculate angle for rotation
            let angle = screen_dir.y.atan2(screen_dir.x);
            // Ensure text is always readable (not upside down)
            let (angle, offset_dir) = if angle.abs() > std::f32::consts::FRAC_PI_2 {
                (angle + std::f32::consts::PI, -screen_perp)
            } else {
                (angle, screen_perp)
            };

            // Offset "up" (perpendicular) based on font size
            let text_pos = mid - offset_dir * (font_size + 5.0);
            
            let galley = ctx.ctx.fonts_mut(|f| {
                f.layout_no_wrap(
                    text.clone(),
                    FontId::new(font_size, FontFamily::Proportional),
                    Color32::LIGHT_GRAY,
                )
            });
            
            // Center the text on text_pos
            let centered_pos = text_pos - galley.size() / 2.0;
            
            // Add a small background for readability
            let bg_rect = galley.rect.translate(centered_pos.to_vec2()).expand(2.0);
            
            // Rotate the background rect manually if needed, but for now just drawing it axis-aligned 
            // might look weird if rotated. 
            // Better to use TextShape with rotation.
            
            // Since we can't easily rotate a rect_filled without a mesh, let's skip the background for rotated text
            // or use a simpler approach.
            
            let mut text_shape = egui::epaint::TextShape::new(text_pos, galley, Color32::LIGHT_GRAY);
            text_shape.angle = angle;
            
            // We need to adjust position because rotation happens around the pos.
            // TextShape draws starting at pos. We want to center it.
            // But TextShape doesn't support centering with rotation easily unless we manually offset.
            // Actually, galley has size.
            
            // Let's try to position it such that it centers.
            // If we rotate around text_pos, we need to offset by half width/height in the rotated frame.
            let half_size = text_shape.galley.size() / 2.0;
            let rotated_offset = Vec2::new(
                half_size.x * angle.cos() - half_size.y * angle.sin(),
                half_size.x * angle.sin() + half_size.y * angle.cos()
            );
            text_shape.pos = text_pos - rotated_offset;

            shapes.push(text_shape.into());
        }

        // Cardinality
        if let Some(card) = &self.cardinality {
            let mid = screen_start + (screen_end - screen_start) * 0.5;
            let font_size = ctx.meta.canvas_to_screen_size(base_font_size_card).max(10.0).min(30.0);
            
            let angle = screen_dir.y.atan2(screen_dir.x);
            let (angle, offset_dir) = if angle.abs() > std::f32::consts::FRAC_PI_2 {
                (angle + std::f32::consts::PI, -screen_perp)
            } else {
                (angle, screen_perp)
            };

            // Offset "down" (opposite to label)
            let text_pos = mid + offset_dir * (font_size + 5.0);
            
            let galley = ctx.ctx.fonts_mut(|f| {
                f.layout_no_wrap(
                    card.clone(),
                    FontId::new(font_size, FontFamily::Proportional),
                    Color32::from_rgb(150, 200, 255),
                )
            });
            
            let mut text_shape = egui::epaint::TextShape::new(text_pos, galley, Color32::from_rgb(150, 200, 255));
            text_shape.angle = angle;
            
            let half_size = text_shape.galley.size() / 2.0;
            let rotated_offset = Vec2::new(
                half_size.x * angle.cos() - half_size.y * angle.sin(),
                half_size.x * angle.sin() + half_size.y * angle.cos()
            );
            text_shape.pos = text_pos - rotated_offset;

            shapes.push(text_shape.into());
        }

        shapes
    }

    fn update(&mut self, state: &EdgeProps<LogEdgeData>) {
        self.selected = state.selected;
        self.label = state.payload.label.clone();
        self.cardinality = state.payload.cardinality.clone();
    }
}
