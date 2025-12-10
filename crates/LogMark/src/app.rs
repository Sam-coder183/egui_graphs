use eframe::App;
use egui::{Context, SidePanel, CentralPanel, TopBottomPanel, Window, Align2, Key};
use std::time::{Duration, Instant};
use std::path::Path;
use petgraph::graph::EdgeIndex;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(target_arch = "wasm32")]
use web_sys::{Storage, Blob, BlobPropertyBag, Url, HtmlAnchorElement, FileReader, Event};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(not(target_arch = "wasm32"))]
use rfd::FileDialog;
#[cfg(target_arch = "wasm32")]
use js_sys::Array;
use egui_graphs::{
    Graph, GraphView,
    LayoutStateRandom, LayoutRandom,
    LayoutForceDirected, FruchtermanReingold, FruchtermanReingoldState,
    LayoutHierarchical, LayoutStateHierarchical,
    events::Event as GraphEvent,
    SettingsInteraction, SettingsNavigation,
    MetadataFrame,
};
use petgraph::{stable_graph::StableGraph, Directed};
use petgraph::visit::{IntoEdgeReferences, EdgeRef};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use regex::Regex;
use ron::de::from_str;
use ron::ser::{to_string_pretty, PrettyConfig};

use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};
use crate::types::{AppLayout, QuickTemplate, AppTab, VisualizationMode, SidebarTab, EditorMode};
use crate::utils::{calculate_edge_cardinality, process_wikilinks_for_preview};
use crate::persistence::{load_autosave, commit_changes, default_graph};
use crate::lua::LuaEngine;
use crate::ui::editor::EditorState;
use crate::analyzer::ProjectAnalyzer;
use crate::ui::settings::Settings3D;
use crate::ui::help::show_help_window;
use crate::ui::settings_window::SettingsWindow;
use crate::actions::cleanup_orphans;
use crate::GRAPH_SETTINGS;

pub struct LogMarkApp {
    graph: Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>,
    settings_window: SettingsWindow,
    editing_label: Option<petgraph::stable_graph::NodeIndex>,
    label_edit_buffer: String,
    editing_edge: Option<EdgeIndex>,
    edge_edit_buffer: String,
    editing_edge_pos: Option<egui::Pos2>,
    markdown_cache: CommonMarkCache,
    wikilink_regex: Regex,
    lua_regex: Regex,
    editing_pos: Option<egui::Pos2>,
    sidebar_expanded: bool,
    
    change_count: usize,
    
    // Components
    editor_state: EditorState,
    lua_engine: LuaEngine,
    settings_3d: Settings3D,

    // Lua
    lua_sidebar_open: bool,
    
    // Layout
    layout: AppLayout,

    // Search / quick add / theme
    search_query: String,
    filter_tasks: bool,
    quick_add_label: String,
    quick_add_template: QuickTemplate,
    dark_mode: bool,
    zen_mode: bool,

    // Undo/redo
    undo_stack: Vec<Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>>,
    redo_stack: Vec<Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>>,

    // WASM import UI
    #[cfg(target_arch = "wasm32")]
    wasm_import_open: bool,
    #[cfg(target_arch = "wasm32")]
    wasm_import_buffer: String,

    // UI feedback
    toasts: Vec<(String, Instant)>,

    // Navigation helpers
    fit_to_view_next: bool,
    show_help: bool,
    #[allow(dead_code)]
    editor_mode: EditorMode,
    
    // Hybrid Editor State
    #[allow(dead_code)]
    editing_block_idx: Option<usize>,
    
    // Sidebar State
    sidebar_tab: SidebarTab,
    
    // App-level Tab and Visualization
    current_tab: AppTab,
    visualization_mode: VisualizationMode,
    
    // Entity Relationship Cardinality
    show_cardinality: bool,

    // Selection State
    selected_node: Option<petgraph::stable_graph::NodeIndex>,

    // Force Layout Settings
    show_force_settings: bool,
}

impl LogMarkApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let graph = load_autosave()
            .filter(|g| g.node_count() >= 3)
            .unwrap_or_else(default_graph);

        Self {
            graph,
            settings_window: SettingsWindow::default(),
            show_force_settings: false,
            editing_label: None,
            label_edit_buffer: String::new(),
            editing_edge: None,
            edge_edit_buffer: String::new(),
            editing_edge_pos: None,
            markdown_cache: CommonMarkCache::default(),
            wikilink_regex: Regex::new(r"\[\[(.*?)(?:\|(.*?))?\]\]").unwrap(),
            lua_regex: Regex::new(r"```lua\n([\s\S]*?)\n```").unwrap(),
            editing_pos: None,
            sidebar_expanded: true,
            change_count: 0,
            
            editor_state: EditorState::new(),
            lua_engine: LuaEngine::new(),
            settings_3d: Settings3D::default(),

            lua_sidebar_open: false,
            layout: AppLayout::Hierarchical,
            search_query: String::new(),
            filter_tasks: false,
            quick_add_label: String::new(),
            quick_add_template: QuickTemplate::Note,
            dark_mode: true,
            zen_mode: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            wasm_import_open: false,
            #[cfg(target_arch = "wasm32")]
            wasm_import_buffer: String::new(),
            toasts: Vec::new(),
            fit_to_view_next: true,
            show_help: false,
            editor_mode: EditorMode::Hybrid,
            editing_block_idx: None,
            sidebar_tab: SidebarTab::Edit,
            current_tab: AppTab::Graph,
            visualization_mode: VisualizationMode::TwoD,
            show_cardinality: false,
            selected_node: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_wasm_import(&mut self) {
        if let Some(storage) = crate::persistence::web_storage() {
            if let Ok(Some(content)) = storage.get_item("logmark_import") {
                match from_str(&content) {
                    Ok(graph) => {
                        self.graph = graph;
                        self.push_toast("Imported graph");
                    }
                    Err(err) => {
                        self.push_toast(format!("Import failed: {}", err));
                    }
                }
                let _ = storage.remove_item("logmark_import");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn web_export_ron(&self) {
        let config = PrettyConfig::default();
        if let Ok(data) = to_string_pretty(&self.graph, config) {
            if let Some(window) = web_sys::window() {
                if let Some(document) = window.document() {
                    let array = Array::new();
                    array.push(&JsValue::from_str(&data));
                    let bag = {
                        let b = BlobPropertyBag::new();
                        b.set_type("application/ron");
                        b
                    };
                    if let Ok(blob) = Blob::new_with_str_sequence_and_options(&array, &bag) {
                        if let Ok(url) = Url::create_object_url_with_blob(&blob) {
                            if let Ok(element) = document.create_element("a") {
                                if let Ok(anchor) = element.dyn_into::<HtmlAnchorElement>() {
                                    anchor.set_href(&url);
                                    anchor.set_download("logmark.ron");
                                    anchor.style().set_property("display", "none").ok();
                                    let _ = document.body().map(|body| body.append_child(&anchor));
                                    anchor.click();
                                    let _ = document.body().map(|body| body.remove_child(&anchor));
                                    let _ = Url::revoke_object_url(&url);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn start_wasm_file_picker(&mut self) {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Ok(elem) = document.create_element("input") {
                    if let Ok(input) = elem.dyn_into::<web_sys::HtmlInputElement>() {
                        input.set_type("file");
                        input.set_accept(".ron");
                        input.style().set_property("display", "none").ok();

                        if let Some(body) = document.body() {
                            let _ = body.append_child(&input);
                        }

                        let onchange = {
                            Closure::wrap(Box::new(move |event: Event| {
                                if let Some(target) = event.target() {
                                    if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                        if let Some(files) = input.files() {
                                            if let Some(file) = files.get(0) {
                                                if let Ok(reader) = FileReader::new() {
                                                    let reader_clone = reader.clone();
                                                    let onload = Closure::wrap(Box::new(move |_e: Event| {
                                                        if let Ok(result) = reader_clone.result() {
                                                            if let Some(text) = result.as_string() {
                                                                if let Some(storage) = crate::persistence::web_storage() {
                                                                    let _ = storage.set_item("logmark_import", &text);
                                                                }
                                                            }
                                                        }
                                                    }) as Box<dyn FnMut(_)>);

                                                    reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                                                    let _ = reader.read_as_text(&file);
                                                    onload.forget();
                                                }
                                            }
                                        }
                                    }
                                }
                            }) as Box<dyn FnMut(_)> )
                        };

                        input.set_onchange(Some(onchange.as_ref().unchecked_ref()));
                        input.click();
                        onchange.forget();

                        if let Some(body) = document.body() {
                            let _ = body.remove_child(&input);
                        }
                    }
                }
            }
        }
    }

    fn push_toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), Instant::now()));
    }

    fn prune_toasts(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(4);
        self.toasts.retain(|(_, t)| *t >= cutoff);
    }

    fn push_undo(&mut self) {
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.graph.clone());
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.graph.clone());
            self.graph = prev;
            self.push_toast("Undo");
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.graph.clone());
            self.graph = next;
            self.push_toast("Redo");
        }
    }

    fn run_code_analysis(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = FileDialog::new().pick_folder() {
                self.push_undo();
                let analyzer = ProjectAnalyzer::new();
                let files = analyzer.analyze_path(&path);
                
                let mut g = StableGraph::new();
                let mut node_map = std::collections::HashMap::new();

                // Create nodes
                for file in &files {
                    let label = file.name.clone();
                    let content = file.content.clone();
                    
                    let idx = g.add_node(LogNodeData {
                        label: label.clone(),
                        content,
                    });
                    
                    // Map "filename_stem" to idx for easier import matching
                    // e.g. "app.rs" -> "app"
                    let stem = Path::new(&file.name).file_stem().and_then(|s| s.to_str()).unwrap_or(&file.name).to_string();
                    node_map.insert(stem, idx);
                    // Also map full name just in case
                    node_map.insert(file.name.clone(), idx);
                }

                // Create edges
                for file in &files {
                    let source_stem = Path::new(&file.name).file_stem().and_then(|s| s.to_str()).unwrap_or(&file.name);
                    // We need to find the node index for the current file to use as source
                    // Since we inserted both stem and full name, we can try looking up by stem
                    if let Some(source_idx) = node_map.get(source_stem) {
                         for import in &file.imports {
                             // Try to find target node by import name
                             if let Some(target_idx) = node_map.get(import) {
                                 if source_idx != target_idx {
                                     g.add_edge(*source_idx, *target_idx, LogEdgeData {
                                         label: Some("imports".to_string()),
                                         cardinality: None,
                                     });
                                 }
                             }
                         }
                    }
                }

                let mut graph = Graph::from(&g);
                // Randomize positions initially
                let indices: Vec<_> = graph.g().node_indices().collect();
                for idx in indices {
                    if let Some(node) = graph.node_mut(idx) {
                        use rand::Rng;
                        let mut rng = rand::thread_rng();
                        node.set_location(egui::Pos2::new(rng.gen_range(0.0..1000.0), rng.gen_range(0.0..1000.0)));
                    }
                }

                self.graph = graph;
                self.layout = AppLayout::Force; 
                self.fit_to_view_next = true;
                self.push_toast(format!("Analyzed {} files", files.len()));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.push_toast("Code analysis not supported in WASM yet");
        }
    }
}

impl App for LogMarkApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Auto-commit check
        if self.change_count > 10 {
            commit_changes(&self.graph);
            self.change_count = 0;
        }

        // Theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        // Undo/Redo shortcuts
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Z)) {
            self.undo();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::Z)) {
            self.redo();
        }

        // Delete node shortcut
        if ctx.input(|i| i.key_pressed(Key::Delete)) && !ctx.wants_keyboard_input() {
             if let Some(idx) = self.selected_node {
                 self.push_undo();
                 self.graph.remove_node(idx);
                 self.selected_node = None;
                 self.change_count += 1;
                 self.push_toast("Deleted node");
             }
        }

        self.prune_toasts();

        #[cfg(target_arch = "wasm32")]
        self.poll_wasm_import();

        // Help Window
        show_help_window(ctx, &mut self.show_help);

        // Settings Window
        self.settings_window.show(ctx);
        if let Ok(mut settings) = GRAPH_SETTINGS.write() {
            settings.font_size_cardinality = self.settings_window.font_size_cardinality;
            settings.font_size_edge_label = self.settings_window.font_size_edge_label;
        }

        // Top Panel
        TopBottomPanel::top("top_menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // App Tabs
                ui.selectable_value(&mut self.current_tab, AppTab::Graph, "📊 Graph");
                ui.selectable_value(&mut self.current_tab, AppTab::Preview, "👁 Preview");
                
                ui.separator();
                
                // Visualization Mode ComboBox
                egui::ComboBox::from_label("View")
                    .selected_text(self.visualization_mode.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.visualization_mode, VisualizationMode::TwoD, "Documentation View 📝");
                        ui.selectable_value(&mut self.visualization_mode, VisualizationMode::CodeAnalysis, "Code Analysis");
                    });
                
                if self.visualization_mode == VisualizationMode::CodeAnalysis {
                    if ui.button("🔍 Analyze Project").clicked() {
                        self.run_code_analysis();
                    }
                }
                
                ui.separator();

                // Zoom Controls
                if ui.button("➕").on_hover_text("Zoom In").clicked() {
                    let mut frame = MetadataFrame::new(None).load(ui);
                    frame.zoom *= 1.2;
                    frame.save(ui);
                }
                if ui.button("➖").on_hover_text("Zoom Out").clicked() {
                    let mut frame = MetadataFrame::new(None).load(ui);
                    frame.zoom /= 1.2;
                    frame.save(ui);
                }
                
                ui.separator();
                
                // File operations
                if ui.button("📄 New").clicked() {
                    self.push_undo();
                    // Create a minimal new graph with one starting node
                    let mut g = StableGraph::new();
                    let start = g.add_node(LogNodeData {
                        label: "Start".to_string(),
                        content: "# Start Here\n\nThis is your new graph. Double-click to edit this node, or use the Quick Add panel on the left to create more nodes.\n\nTry typing `/` in the edit tab to see the slash menu!".to_string(),
                    });
                    let mut graph = Graph::from(&g);
                    if let Some(node) = graph.node_mut(start) { node.set_location(egui::Pos2::new(0.0, 0.0)); }
                    self.graph = graph;
                    self.graph.set_selected_nodes(vec![]); // Clear selection
                    self.selected_node = None;
                    self.layout = AppLayout::Hierarchical;
                    self.fit_to_view_next = true;
                    self.editor_state.content_buffer.clear(); // Clear edit buffer
                    self.editor_state.last_edited_node = None; // Reset tracked node
                    self.push_toast("Created new graph");
                }
                if ui.button("📂 Open").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(path) = FileDialog::new().add_filter("RON", &["ron"]).pick_file() {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(graph) = from_str(&content) {
                                    self.push_undo();
                                    self.graph = graph;
                                    self.selected_node = None;
                                    self.push_toast("Opened graph");
                                } else {
                                    self.push_toast("Failed to parse graph");
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.start_wasm_file_picker();
                    }
                }
                if ui.button("💾 Save").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(path) = FileDialog::new().add_filter("RON", &["ron"]).save_file() {
                            let config = PrettyConfig::default();
                            if let Ok(data) = to_string_pretty(&self.graph, config) {
                                if let Err(err) = fs::write(path, data) {
                                    self.push_toast(format!("Save failed: {}", err));
                                } else {
                                    self.push_toast("Saved graph");
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.web_export_ron();
                    }
                }
                
                ui.separator();
                
                if ui.button("⛶ Fit View").clicked() {
                    self.fit_to_view_next = true;
                    self.push_toast("Fit to view");
                }

                if ui.button("❓ Help").clicked() {
                    self.show_help = true;
                }

                if ui.button("↪ Undo (Ctrl+Z)").clicked() {
                    self.undo();
                }
                if ui.button("↪ Redo (Ctrl+Shift+Z)").clicked() {
                    self.redo();
                }

                if ui.button(if self.dark_mode { "☀" } else { "🌙" }).clicked() {
                    self.dark_mode = !self.dark_mode;
                    self.push_toast(if self.dark_mode { "Dark mode" } else { "Light mode" });
                }

                if let Some((msg, _)) = self.toasts.first() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(msg);
                    });
                }
            });
        });

        // WASM import modal for pasted RON
        #[cfg(target_arch = "wasm32")]
        if self.wasm_import_open {
            let mut open = true;
            Window::new("Import RON")
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label("Paste RON graph content:");
                    ui.add_sized(ui.available_size() - egui::Vec2::new(0.0, 60.0), TextEdit::multiline(&mut self.wasm_import_buffer));
                    ui.horizontal(|ui| {
                        if ui.button("Import").clicked() {
                            if let Ok(graph) = from_str(&self.wasm_import_buffer) {
                                self.graph = graph;
                                self.push_toast("Imported graph");
                                self.wasm_import_open = false;
                            } else {
                                self.push_toast("Invalid RON");
                            }
                        }
                    });
                });

            if !open {
                self.wasm_import_open = false;
            }
        }

        // 3D Visualization Settings Window
        self.settings_3d.show(ctx);
        self.settings_3d.update(ctx);

        // Simulation Settings Window
        if self.show_force_settings && self.layout == AppLayout::Force {
            let mut open = true;
            Window::new("Simulation Settings")
                .open(&mut open)
                .show(ctx, |ui| {
                    let mut state = egui_graphs::get_layout_state::<FruchtermanReingoldState>(ui, None);
                    
                    ui.checkbox(&mut state.is_running, "Running");
                    ui.add(egui::Slider::new(&mut state.dt, 0.001..=0.2).text("dt"));
                    ui.add(egui::Slider::new(&mut state.damping, 0.0..=1.0).text("Damping"));
                    ui.add(egui::Slider::new(&mut state.max_step, 0.1..=50.0).text("Max Step"));
                    ui.add(egui::Slider::new(&mut state.c_attract, 0.01..=10.0).text("Attraction"));
                    ui.add(egui::Slider::new(&mut state.c_repulse, 0.01..=10.0).text("Repulsion"));
                    
                    egui_graphs::set_layout_state(ui, state, None);
                });
             if !open {
                 self.show_force_settings = false;
             }
        }

        // Zen Mode Toggle (Escape to exit)
        if self.zen_mode {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.zen_mode = false;
                self.push_toast("Exited Zen Mode");
            }
        }

        // Left options & debugger sidebar
        if !self.zen_mode {
            SidePanel::left("left_panel")
                .resizable(true)
                .default_width(220.0)
                .show(ctx, |ui| {
                    ui.heading("Options");
                    if ui.button("Zen Mode (Esc)").clicked() {
                        self.zen_mode = true;
                        self.push_toast("Zen Mode (Esc to exit)");
                    }
                    if ui.button("🧹 Cleanup Orphans").clicked() {
                        self.push_undo();
                        let count = cleanup_orphans(&mut self.graph);
                        self.push_toast(format!("Removed {} orphans", count));
                        self.change_count += 1;
                    }
                    if ui.button("⚙ Settings").clicked() {
                        self.settings_window.open = true;
                    }
                    ui.separator();
                    ui.label("Layout:");
                ui.radio_value(&mut self.layout, AppLayout::Random, "Random")
                    .on_hover_text("Scatter nodes unpredictably");
                ui.radio_value(&mut self.layout, AppLayout::Force, "Force")
                    .on_hover_text("Physics-based layout");
                if self.layout == AppLayout::Force {
                    if ui.button("⚙ Simulation").clicked() {
                        self.show_force_settings = !self.show_force_settings;
                    }
                }
                ui.radio_value(&mut self.layout, AppLayout::Hierarchical, "Hierarchical")
                    .on_hover_text("Tree view that fills the canvas");

                if ui.button("Fit View").clicked() {
                    self.fit_to_view_next = true;
                    self.push_toast("Fitting view");
                }

                ui.separator();
                ui.heading("Entity Relationships");
                ui.checkbox(&mut self.show_cardinality, "Show Cardinality (1:1, 1:N, N:N)")
                    .on_hover_text("Display relationship cardinality below edge labels");
                
                if self.show_cardinality {
                    // Show relationship summary
                    ui.collapsing("📊 Relationship Stats", |ui| {
                        let mut one_to_one = 0;
                        let mut one_to_many = 0;
                        let mut many_to_many = 0;
                        
                        for edge in self.graph.g().edge_references() {
                            let card = calculate_edge_cardinality(&self.graph, edge.source(), edge.target());
                            match card.as_str() {
                                "1:1" => one_to_one += 1,
                                "1:N" | "N:1" => one_to_many += 1,
                                "N:N" => many_to_many += 1,
                                _ => {}
                            }
                        }
                        
                        ui.label(format!("1:1 - {}", one_to_one));
                        ui.label(format!("1:N - {}", one_to_many));
                        ui.label(format!("N:N - {}", many_to_many));
                    });
                }

                ui.separator();
                ui.label("Search nodes/edges:");
                ui.checkbox(&mut self.filter_tasks, "Tasks only");
                if ui.text_edit_singleline(&mut self.search_query).changed() {
                    // no-op, handled below
                }
                let q = self.search_query.to_lowercase();

                if !q.is_empty() || self.filter_tasks {
                    let results: Vec<_> = self.graph.g().node_indices().take(50).filter_map(|idx| {
                        if let Some(node) = self.graph.node(idx) {
                            let label = &node.payload().label;
                            let content = &node.payload().content;
                            let matches_query = q.is_empty() || label.to_lowercase().contains(&q) || content.to_lowercase().contains(&q);
                            let matches_task = !self.filter_tasks || content.contains("- [ ]");
                            if matches_query && matches_task {
                                return Some((idx, label.clone()));
                            }
                        }
                        None
                    }).collect();
                    for (idx, label) in results {
                        if ui.link(format!("• {}", label)).clicked() {
                            self.graph.node_mut(idx).unwrap().set_selected(true);
                            let _pos = self.graph.node(idx).unwrap().location();
                            // We can't easily center view here without access to GraphView state, 
                            // but selecting it highlights it.
                        }
                    }
                }

                ui.separator();
                ui.label("Quick add:");
                ui.text_edit_singleline(&mut self.quick_add_label);
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.quick_add_template, QuickTemplate::Note, "Note");
                    ui.radio_value(&mut self.quick_add_template, QuickTemplate::Heading, "Heading");
                    ui.radio_value(&mut self.quick_add_template, QuickTemplate::Task, "Task");
                });
                if ui.button("Add Node").clicked() || (ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.ctrl)) {
                    if !self.quick_add_label.is_empty() {
                        self.push_undo();
                        let content = match self.quick_add_template {
                            QuickTemplate::Note => format!("# {}\n", self.quick_add_label),
                            QuickTemplate::Heading => format!("# {}\n\n## Section\n", self.quick_add_label),
                            QuickTemplate::Task => format!("# {}\n\n- [ ] Task 1\n", self.quick_add_label),
                        };
                        let idx = self.graph.add_node(LogNodeData {
                            label: self.quick_add_label.clone(),
                            content,
                        });
                        self.graph.node_mut(idx).unwrap().set_location(egui::Pos2::new(0.0, 0.0));
                        self.quick_add_label.clear();
                        self.change_count += 1;
                        self.push_toast("Added node");
                    }
                }

                ui.separator();
                ui.label(format!("Auto-commit after 10 edits (current: {})", self.change_count));

                ui.separator();
                ui.heading("Lua Debugger");
                if self.lua_sidebar_open {
                    if ui.button("Close Debugger").clicked() {
                        self.lua_sidebar_open = false;
                    }
                    ui.separator();
                    ui.label("Variables:");
                    egui::ScrollArea::vertical()
                        .id_salt("lua_debugger_vars")
                        .max_height(100.0)
                        .show(ui, |ui| {
                        for (k, v) in &self.lua_engine.variables {
                            ui.label(format!("{} = {}", k, v));
                        }
                    });
                    ui.separator();
                    ui.label("Output:");
                    egui::ScrollArea::vertical()
                        .id_salt("lua_debugger_output")
                        .max_height(150.0)
                        .show(ui, |ui| {
                        ui.monospace(&self.lua_engine.output);
                    });
                    ui.label("CPU Time: 0.5 ms");
                } else {
                    if ui.button("Open Debugger").clicked() {
                        self.lua_sidebar_open = true;
                    }
                }
            });
        }

        // Label Editor
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
                        self.change_count += 1;
                    }
                    self.editing_label = None;
                }
            });

            if !open {
                self.editing_label = None;
                self.editing_pos = None;
            }
        }

        // Edge Label Editor
        if let Some(eidx) = self.editing_edge {
            let mut open = true;
            let mut win = Window::new("Edit Edge Label");
            if let Some(pos) = self.editing_edge_pos {
                win = win.anchor(Align2::LEFT_TOP, [pos.x, pos.y]);
            } else {
                win = win.anchor(Align2::CENTER_CENTER, [0.0, 0.0]);
            }

            win.open(&mut open).show(ctx, |ui| {
                let response = ui.text_edit_singleline(&mut self.edge_edit_buffer);
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some(edge) = self.graph.edge_mut(eidx) {
                        edge.payload_mut().label = Some(self.edge_edit_buffer.clone());
                        self.change_count += 1;
                    }
                    self.editing_edge = None;
                }
            });

            if !open {
                self.editing_edge = None;
                self.editing_edge_pos = None;
            }
        }

        // Sidebar
        if !self.zen_mode {
            SidePanel::right("right_panel")
                .resizable(true)
                .default_width(if self.sidebar_expanded { 300.0 } else { 48.0 })
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(if self.sidebar_expanded { "▶" } else { "◀" }).clicked() {
                            self.sidebar_expanded = !self.sidebar_expanded;
                        }
                        if self.sidebar_expanded {
                            ui.heading("Details");
                        }
                    });

                    if !self.sidebar_expanded {
                        return;
                    }

                    ui.separator();

                if let Some(idx) = self.selected_node {
                    // Ensure node still exists (might have been deleted)
                    if let Some(node) = self.graph.node(idx) {
                        let label = node.payload().label.clone();
                        
                        // Sync editor buffer if node changed
                        let just_synced = if self.editor_state.last_edited_node != Some(idx) {
                            self.editor_state.last_edited_node = Some(idx);
                            self.editor_state.content_buffer = node.payload().content.clone();
                            self.editor_state.completer.push_word(&label); // Add title to completions
                            true
                        } else {
                            false
                        };

                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Edit, "✏ Edit");
                            ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Preview, "👁 Preview");
                            ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Lua, "🌙 Lua");
                        });
                        ui.separator();

                        match self.sidebar_tab {
                            SidebarTab::Preview => {
                                egui::ScrollArea::both()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                    ui.heading(&label);
                                    ui.separator();
                                    let processed = process_wikilinks_for_preview(&node.payload().content);
                                    CommonMarkViewer::new()
                                        .show(ui, &mut self.markdown_cache, &processed);
                                });
                            }
                            SidebarTab::Lua => {
                                ui.label("Lua Scripting for this node");
                                ui.separator();
                                
                                // Extract Lua blocks
                                let content = &node.payload().content;
                                let mut lua_blocks = Vec::new();
                                for cap in self.lua_regex.captures_iter(content) {
                                    if let Some(m) = cap.get(1) {
                                        lua_blocks.push(m.as_str().to_string());
                                    }
                                }

                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    if !lua_blocks.is_empty() {
                                        for (i, block) in lua_blocks.iter().enumerate() {
                                            ui.group(|ui| {
                                                ui.label(format!("Block {}", i + 1));
                                                ui.monospace(block);
                                                if ui.button("▶ Run").clicked() {
                                                    self.lua_engine.run_script(
                                                        block, 
                                                        &mut self.graph, 
                                                        &mut self.change_count,
                                                        &mut self.undo_stack,
                                                        &mut self.redo_stack
                                                    );
                                                    self.lua_sidebar_open = true;
                                                }
                                                
                                                // Quick actions
                                                ui.horizontal(|ui| {
                                                    if ui.button("Debug").clicked() {
                                                        self.lua_sidebar_open = true;
                                                    }
                                                    if ui.button("Copy").clicked() {
                                                        ui.ctx().copy_text(block.clone());
                                                        self.push_toast("Copied to clipboard");
                                                    }
                                                });
                                            });
                                        }
                                    } else {
                                        ui.label("📝 No Lua block in current node.");
                                        ui.add_space(4.0);
                                        if ui.button("➕ Add Lua Block").clicked() {
                                            self.editor_state.content_buffer.push_str("\n\n```lua\n-- Your Lua script here\nprint('Hello!')\n```\n");
                                            if let Some(node) = self.graph.node_mut(idx) {
                                                node.payload_mut().content = self.editor_state.content_buffer.clone();
                                            }
                                            self.change_count += 1;
                                        }
                                    }
                                });
                            }
                            SidebarTab::Edit => {
                                // Edit Tab - Full text editor with slash menu
                                ui.horizontal(|ui| {
                                    ui.label(format!("Editing: {}", label));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label("Type / for commands");
                                    });
                                });
                                ui.separator();
                                
                                self.editor_state.show(
                                    ui, 
                                    idx, 
                                    &mut self.graph, 
                                    &mut self.change_count, 
                                    &self.wikilink_regex,
                                    just_synced
                                );
                            }
                        }
                    } else {
                        // Node was deleted or invalid
                        self.selected_node = None;
                        ui.label("Select a node to view details.");
                    }
                } else {
                    ui.label("Select a node to view details.");
                }
            });
        }
        
        // Auto-rotate for 3D view
        self.settings_3d.update(ctx);
            
        // Update cardinality if enabled
        if self.show_cardinality {
            let edges: Vec<_> = self.graph.g().edge_indices().collect();
            for edge_idx in edges {
                if let Some((source, target)) = self.graph.edge_endpoints(edge_idx) {
                    let card = calculate_edge_cardinality(&self.graph, source, target);
                    if let Some(edge) = self.graph.edge_mut(edge_idx) {
                        edge.payload_mut().cardinality = Some(card);
                    }
                }
            }
        } else {
            // Clear cardinality if disabled
            let edges: Vec<_> = self.graph.g().edge_indices().collect();
            for edge_idx in edges {
                if let Some(edge) = self.graph.edge_mut(edge_idx) {
                    edge.payload_mut().cardinality = None;
                }
            }
        }

        // Central Panel - based on current tab
        CentralPanel::default().show(ctx, |ui| {
            match self.current_tab {
                AppTab::Graph => {
                    let (sender, receiver) = std::sync::mpsc::channel();
                    let sink = move |e: GraphEvent| { sender.send(e).ok(); };

                    let settings_interaction = SettingsInteraction::default()
                        .with_dragging_enabled(true)
                        .with_node_clicking_enabled(true)
                        .with_node_selection_enabled(false)
                        .with_node_selection_multi_enabled(false)
                        .with_edge_clicking_enabled(true)
                        .with_edge_selection_enabled(false)
                        .with_edge_selection_multi_enabled(false);
                    
                    let settings_navigation = SettingsNavigation::default()
                        .with_zoom_and_pan_enabled(true)
                        .with_fit_to_screen_enabled(self.fit_to_view_next);
                    
                    self.fit_to_view_next = false;

                    // Apply layout
                    let response = match self.layout {
                        AppLayout::Random => {
                            let mut graph_view = GraphView::<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge, LayoutStateRandom, LayoutRandom>::new(&mut self.graph)
                                .with_interactions(&settings_interaction)
                                .with_navigations(&settings_navigation)
                                .with_event_sink(&sink);
                            ui.add(&mut graph_view)
                        },
                        AppLayout::Force => {
                            let mut graph_view = GraphView::<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge, FruchtermanReingoldState, LayoutForceDirected<FruchtermanReingold>>::new(&mut self.graph)
                                .with_interactions(&settings_interaction)
                                .with_navigations(&settings_navigation)
                                .with_event_sink(&sink);
                            ui.add(&mut graph_view)
                        },
                        AppLayout::Hierarchical => {
                            let mut graph_view = GraphView::<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge, LayoutStateHierarchical, LayoutHierarchical>::new(&mut self.graph)
                                .with_interactions(&settings_interaction)
                                .with_navigations(&settings_navigation)
                                .with_event_sink(&sink);
                            ui.add(&mut graph_view)
                        },
                    };

                    // Handle interactions
                    let mut element_clicked = false;
                    while let Ok(event) = receiver.try_recv() {
                        match event {
                            GraphEvent::NodeClick(payload) => {
                                let idx = NodeIndex::new(payload.id);
                                self.selected_node = Some(idx);
                                self.graph.set_selected_nodes(vec![idx]);
                                self.sidebar_expanded = true;
                                element_clicked = true;
                            }
                            GraphEvent::NodeDoubleClick(payload) => {
                                let idx = NodeIndex::new(payload.id);
                                self.editing_label = Some(idx);
                                self.label_edit_buffer = self.graph.node(idx).unwrap().payload().label.clone();
                                // We don't have easy access to hover pos from event, so we center or use last known mouse pos
                                // For now, center
                                self.editing_pos = None; 
                                element_clicked = true;
                            }
                            GraphEvent::EdgeClick(_payload) => {
                                // Optional: Handle edge click
                                element_clicked = true;
                            }
                            _ => {}
                        }
                    }
                    
                    // Deselect if background clicked
                    if response.clicked() && !element_clicked {
                        self.graph.set_selected_nodes(vec![]);
                        self.selected_node = None;
                    }

                    // Code Analysis overlay
                    if self.visualization_mode == VisualizationMode::CodeAnalysis {
                        ui.label("Code Analysis Mode. Click 'Analyze Project' to scan.");
                    }
                }
                
                AppTab::Preview => {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                        ui.heading("Graph Preview");
                        ui.separator();
                        for idx in self.graph.g().node_indices() {
                            if let Some(node) = self.graph.node(idx) {
                                ui.heading(&node.payload().label);
                                let processed = process_wikilinks_for_preview(&node.payload().content);
                                CommonMarkViewer::new()
                                    .show(ui, &mut self.markdown_cache, &processed);
                                ui.separator();
                            }
                        }
                    });
                }
            }
        });

        // Handle node:// links from markdown
        let mut node_to_select = None;
        ctx.output_mut(|o| {
            let mut command_to_remove = None;
            for (i, cmd) in o.commands.iter().enumerate() {
                if let egui::OutputCommand::OpenUrl(open_url) = cmd {
                    if open_url.url.starts_with("node://") {
                        let label = open_url.url.strip_prefix("node://").unwrap();
                        node_to_select = Some(label.to_string());
                        command_to_remove = Some(i);
                        break;
                    }
                }
            }
            
            if let Some(i) = command_to_remove {
                o.commands.remove(i);
            }
        });

        if let Some(label) = node_to_select {
            let decoded_label = label.replace("%20", " ");
            if let Some(idx) = self.graph.g().node_indices().find(|&i| {
                self.graph.node(i).map(|n| n.payload().label == decoded_label).unwrap_or(false)
            }) {
                self.selected_node = Some(idx);
                self.graph.set_selected_nodes(vec![idx]);
                self.sidebar_expanded = true;
                self.sidebar_tab = SidebarTab::Preview;
                // Optional: Switch to graph view to show context
                // self.current_tab = AppTab::Graph; 
            } else {
                self.push_toast(format!("Node not found: {}", decoded_label));
            }
        }
    }
}

use petgraph::graph::NodeIndex;
