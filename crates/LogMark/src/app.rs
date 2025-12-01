use eframe::App;
use egui::{Context, SidePanel, CentralPanel, TextEdit, Window, Align2, Key};
use egui_graphs::{Graph, GraphView, SettingsInteraction, LayoutStateRandom, LayoutRandom};
use petgraph::{stable_graph::StableGraph, Directed};
use petgraph::visit::EdgeRef;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use regex::Regex;

use crate::graph::{LogNode, LogEdge, LogNodeData};

/// Type alias for GraphView with our custom node/edge types and random layout
type LogMarkGraphView<'a> = GraphView<'a, LogNodeData, (), Directed, u32, LogNode, LogEdge, LayoutStateRandom, LayoutRandom>;

pub struct LogMarkApp {
    graph: Graph<LogNodeData, (), Directed, u32, LogNode, LogEdge>,
    editing_label: Option<petgraph::stable_graph::NodeIndex>,
    label_edit_buffer: String,
    markdown_cache: CommonMarkCache,
    
    // For wikilinks detection
    wikilink_regex: Regex,

    // Where to place the inline label editor (screen coords)
    editing_pos: Option<egui::Pos2>,

    // Sidebar collapsed/expanded
    sidebar_expanded: bool,
}

impl LogMarkApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut g = StableGraph::new();
        
        let idx1 = g.add_node(LogNodeData { 
            label: "Home".to_string(), 
            content: "# Welcome to LogMark\n\nThis is a graph-based note taking app.\n\nTry adding a link like [[Ideas]]".to_string() 
        });
        let idx2 = g.add_node(LogNodeData { 
            label: "Ideas".to_string(), 
            content: "## My Ideas\n\n- [ ] Build a spaceship\n- [ ] Learn Rust".to_string() 
        });
        
        g.add_edge(idx1, idx2, ());

        let mut graph = Graph::from(&g);
        
        // Initial layout
        if let Some(node) = graph.node_mut(idx1) {
            node.set_location(egui::Pos2::new(0.0, 0.0));
        }
        if let Some(node) = graph.node_mut(idx2) {
            node.set_location(egui::Pos2::new(100.0, 100.0));
        }

        Self {
            graph,
            editing_label: None,
            label_edit_buffer: String::new(),
            markdown_cache: CommonMarkCache::default(),
            wikilink_regex: Regex::new(r"\[\[(.*?)\]\]").unwrap(),
            editing_pos: None,
            sidebar_expanded: true,
        }
    }

    fn handle_wikilinks(&mut self, node_idx: petgraph::stable_graph::NodeIndex) {
        let content = self.graph.node(node_idx).unwrap().payload().content.clone();
        
        // Find all wikilinks
        let mut new_links = Vec::new();
        for cap in self.wikilink_regex.captures_iter(&content) {
            if let Some(m) = cap.get(1) {
                new_links.push(m.as_str().to_string());
            }
        }

        // For each link, ensure a node exists and an edge exists
        let mut target_indices = Vec::new();
        
        for link_label in new_links {
            // Check if node exists
            let mut target_idx = None;
            for idx in self.graph.g().node_indices() {
                if self.graph.node(idx).unwrap().payload().label == link_label {
                    target_idx = Some(idx);
                    break;
                }
            }

            let target_idx = match target_idx {
                Some(idx) => idx,
                None => {
                    // Create new node
                    let new_node_data = LogNodeData {
                        label: link_label.clone(),
                        content: format!("# {}", link_label),
                    };
                    let idx = self.graph.add_node(new_node_data);
                    // Position it somewhere near the source (randomly or fixed offset for now)
                    // In a real app, we'd run a layout algorithm or place it smarter
                    let source_pos = self.graph.node(node_idx).unwrap().location();
                    self.graph.node_mut(idx).unwrap().set_location(source_pos + egui::Vec2::new(50.0, 50.0));
                    idx
                }
            };
            target_indices.push(target_idx);
        }

        // Sync edges: Add edges that don't exist
        for target_idx in target_indices {
            if target_idx == node_idx { continue; } // Don't link to self for now
            
            let mut edge_exists = false;
            for edge in self.graph.g().edges(node_idx) {
                if edge.target() == target_idx {
                    edge_exists = true;
                    break;
                }
            }

            if !edge_exists {
                self.graph.add_edge(node_idx, target_idx, ());
            }
        }
    }
}

impl App for LogMarkApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Double-click handling is done via the GraphView response below.

        // Label Editor Window
        if let Some(idx) = self.editing_label {
            let mut open = true;

            let mut win = Window::new("Edit Label");
            if let Some(pos) = self.editing_pos {
                win = win.anchor(Align2::LEFT_TOP, [pos.x, pos.y]);
            } else {
                win = win.anchor(Align2::CENTER_CENTER, [0.0, 0.0]);
            }

            win.open(&mut open).show(ctx, |ui| {
                let response = ui.text_edit_singleline(&mut self.label_edit_buffer);
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some(node) = self.graph.node_mut(idx) {
                        node.payload_mut().label = self.label_edit_buffer.clone();
                    }
                    self.editing_label = None;
                    self.editing_pos = None;
                }
            });

            if !open {
                self.editing_label = None;
                self.editing_pos = None;
            }
        }

        // Sidebar
        SidePanel::right("right_panel")
            .resizable(true)
            .default_width(if self.sidebar_expanded { 300.0 } else { 48.0 })
            .show(ctx, |ui| {
                // Collapse/Expand button
                ui.horizontal(|ui| {
                    if ui.button(if self.sidebar_expanded { "«" } else { "»" }).clicked() {
                        self.sidebar_expanded = !self.sidebar_expanded;
                    }
                    ui.heading("Sidebar");
                });

                if !self.sidebar_expanded {
                    return;
                }

                if let Some(idx) = self.graph.selected_nodes().first() {
                    let idx = *idx; // Copy index to avoid borrow checker issues
                    
                    ui.heading("Node Content");
                    ui.separator();
                    
                    // We need to get the content, edit it, and put it back.
                    // To avoid holding a mutable borrow on graph while drawing UI that might need it,
                    // we clone the content string.
                    let mut content = self.graph.node(idx).unwrap().payload().content.clone();
                    let label = self.graph.node(idx).unwrap().payload().label.clone();

                    ui.label(format!("Editing: {}", label));
                    
                    let response = ui.add_sized(
                        ui.available_size() - egui::Vec2::new(0.0, 200.0), // Leave space for preview
                        TextEdit::multiline(&mut content)
                            .desired_width(f32::INFINITY)
                            .code_editor()
                    );

                    if response.changed() {
                        // Update content in graph
                        if let Some(node) = self.graph.node_mut(idx) {
                            node.payload_mut().content = content.clone();
                        }
                        // Handle wikilinks
                        self.handle_wikilinks(idx);
                    }

                    // Slash command simple popup when typing '/'
                    if content.ends_with('/') {
                        // Draw a tiny suggestion window anchored near the editor rect
                        let popup_pos = response.rect.max;
                        let mut open_popup = true;
                        Window::new("slash_popup")
                            .open(&mut open_popup)
                            .collapsible(false)
                            .resizable(false)
                            .anchor(Align2::LEFT_TOP, [popup_pos.x + 6.0, popup_pos.y + 6.0])
                            .show(ctx, |ui| {
                                ui.label("Insert:");
                                if ui.button("Heading 1").clicked() {
                                    // replace trailing '/' with '# '
                                    content.truncate(content.len().saturating_sub(1));
                                    content.push_str("# ");
                                    if let Some(node) = self.graph.node_mut(idx) {
                                        node.payload_mut().content = content.clone();
                                    }
                                }
                                if ui.button("Heading 2").clicked() {
                                    content.truncate(content.len().saturating_sub(1));
                                    content.push_str("## ");
                                    if let Some(node) = self.graph.node_mut(idx) {
                                        node.payload_mut().content = content.clone();
                                    }
                                }
                                if ui.button("Bullet").clicked() {
                                    content.truncate(content.len().saturating_sub(1));
                                    content.push_str("- ");
                                    if let Some(node) = self.graph.node_mut(idx) {
                                        node.payload_mut().content = content.clone();
                                    }
                                }
                                if ui.button("To-do").clicked() {
                                    content.truncate(content.len().saturating_sub(1));
                                    content.push_str("- [ ] ");
                                    if let Some(node) = self.graph.node_mut(idx) {
                                        node.payload_mut().content = content.clone();
                                    }
                                }
                            });
                    }

                    ui.separator();
                    ui.heading("Preview");
                    
                    egui::ScrollArea::vertical().show(ui, |ui| {
                            CommonMarkViewer::new()
                            .show(ui, &mut self.markdown_cache, &content);
                    });

                } else {
                    ui.label("Select a node to edit its content.");
                }
            });

        // Graph View
        CentralPanel::default().show(ctx, |ui| {
            let mut widget: LogMarkGraphView<'_> = GraphView::new(&mut self.graph);
            widget = widget.with_interactions(
                &SettingsInteraction::default()
                    .with_dragging_enabled(true)
                    .with_node_selection_enabled(true)
                    .with_node_selection_multi_enabled(false)
            );

            let resp = ui.add(&mut widget);
            // If the graph widget reports a double click, begin inline label editing for selected node
            if resp.double_clicked() {
                if let Some(idx) = self.graph.selected_nodes().first() {
                    self.editing_label = Some(*idx);
                    self.label_edit_buffer = self.graph.node(*idx).unwrap().payload().label.clone();
                    // Anchor editor to pointer position if available
                    self.editing_pos = resp.hover_pos();
                }
            }
        });
    }
}
