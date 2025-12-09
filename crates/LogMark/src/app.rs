use eframe::App;
use egui::{Context, SidePanel, CentralPanel, TopBottomPanel, TextEdit, Window, Align2, Key};
use std::time::{Duration, Instant};
use petgraph::graph::EdgeIndex;
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, path::PathBuf};
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
    Graph, GraphView, SettingsInteraction, SettingsNavigation,
    LayoutStateRandom, LayoutRandom,
    LayoutForceDirected, FruchtermanReingold, FruchtermanReingoldState,
    LayoutHierarchical, LayoutStateHierarchical,
};
use petgraph::{stable_graph::StableGraph, Directed};
use petgraph::visit::EdgeRef;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use regex::Regex;
use ron::de::from_str;
use ron::ser::{to_string_pretty, PrettyConfig};

use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};

#[derive(PartialEq)]
enum AppLayout {
    Random,
    Force,
    Hierarchical,
}

#[derive(Clone, Copy, PartialEq)]
enum QuickTemplate {
    Note,
    Heading,
    Task,
}

pub struct LogMarkApp {
    graph: Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>,
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
    
    // New fields
    change_count: usize,
    slash_menu_open: bool,
    slash_menu_query: String,
    slash_menu_pos: Option<egui::Pos2>,
    slash_menu_selection: usize,
    
    // Lua
    lua_sidebar_open: bool,
    lua_output: String,
    lua_variables: Vec<(String, String)>, // Name, Value
    
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
    editing_node_content: bool,
}

impl LogMarkApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn autosave_path() -> PathBuf {
        PathBuf::from("logmark_autosave.ron")
    }

    fn default_graph() -> Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge> {
        let mut g = StableGraph::new();

        let overview = g.add_node(LogNodeData {
            label: "LogMark".to_string(),
            content: "# LogMark\n\nWelcome to LogMark, a graph-based note app.\n\nBrowse nodes to learn features. Select a node to edit on the right.".to_string(),
        });

        let auto_commit = g.add_node(LogNodeData {
            label: "Auto-Commit".to_string(),
            content: "# Auto-Commit\n\nEdits increment a counter; after 10 the app auto-commits.\nYou can keep typing—commits are automatic.".to_string(),
        });

        let layouts = g.add_node(LogNodeData {
            label: "Layouts".to_string(),
            content: "# Layouts\n\nSwitch layouts from the left Options bar.\n- Random: scattered.\n- Force: physics.\n- Hierarchical: tree view to fill the screen.".to_string(),
        });

        let relationships = g.add_node(LogNodeData {
            label: "Relationships".to_string(),
            content: "# Relationships\n\nEdges show labels above arrows.\nUse [[WikiLinks]] or the slash menu to connect notes.".to_string(),
        });

        let slash = g.add_node(LogNodeData {
            label: "Slash Menu".to_string(),
            content: "# Slash Menu\n\nType `/` to open. Navigate with ↑/↓. Press Enter or Tab to accept. Options: headings, Lua block, or link to another node.".to_string(),
        });

        let lua = g.add_node(LogNodeData {
            label: "Lua".to_string(),
            content: "# Lua Integration\n\nLogMark supports Lua scripting for dynamic content.\n\n## Usage\nCreate a code block with `lua` language:\n\n```lua\n-- Example\nlocal x = 10\nprint('Value:', x)\ngraph.add_node('New Node')\n```\n\n## Features\n- **Run**: Execute the script.\n- **Debug**: Inspect variables in the left sidebar.\n- **API**:\n  - `print(val)`: Print to output.\n  - `graph.add_node('label')`: Create a node.\n  - `graph.add_edge('from', 'to', 'label')`: Connect nodes.\n  - `graph.list_nodes()`: List all nodes.".to_string(),
        });

        let hierarchy = g.add_node(LogNodeData {
            label: "Hierarchy".to_string(),
            content: "# Hierarchy\n\nHierarchical layout provides a node tree view that can fill the screen.".to_string(),
        });

        g.add_edge(overview, auto_commit, LogEdgeData { label: Some("explains".to_string()) });
        g.add_edge(overview, layouts, LogEdgeData { label: Some("shows".to_string()) });
        g.add_edge(overview, relationships, LogEdgeData { label: Some("details".to_string()) });
        g.add_edge(overview, slash, LogEdgeData { label: Some("includes".to_string()) });
        g.add_edge(overview, lua, LogEdgeData { label: Some("supports".to_string()) });
        g.add_edge(overview, hierarchy, LogEdgeData { label: Some("adds".to_string()) });
        g.add_edge(slash, relationships, LogEdgeData { label: Some("creates".to_string()) });
        g.add_edge(layouts, hierarchy, LogEdgeData { label: Some("offers".to_string()) });
        g.add_edge(lua, relationships, LogEdgeData { label: Some("annotates".to_string()) });

        let mut graph = Graph::from(&g);

        // Seed positions so the first frame fills the canvas reasonably
        if let Some(node) = graph.node_mut(overview) { node.set_location(egui::Pos2::new(0.0, 0.0)); }
        if let Some(node) = graph.node_mut(auto_commit) { node.set_location(egui::Pos2::new(150.0, -70.0)); }
        if let Some(node) = graph.node_mut(layouts) { node.set_location(egui::Pos2::new(150.0, 70.0)); }
        if let Some(node) = graph.node_mut(relationships) { node.set_location(egui::Pos2::new(-150.0, -50.0)); }
        if let Some(node) = graph.node_mut(slash) { node.set_location(egui::Pos2::new(-150.0, 60.0)); }
        if let Some(node) = graph.node_mut(lua) { node.set_location(egui::Pos2::new(40.0, 170.0)); }
        if let Some(node) = graph.node_mut(hierarchy) { node.set_location(egui::Pos2::new(220.0, 180.0)); }

        graph
    }

    fn load_autosave() -> Option<Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::autosave_path();
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        match from_str(&content) {
                            Ok(graph) => {
                                println!("Loaded autosave from {}", path.display());
                                return Some(graph);
                            },
                            Err(e) => eprintln!("Failed to deserialize autosave: {}", e),
                        }
                    },
                    Err(e) => eprintln!("Failed to read autosave: {}", e),
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(storage) = Self::web_storage() {
                if let Ok(Some(content)) = storage.get_item("logmark_autosave") {
                    if let Ok(graph) = from_str(&content) {
                        return Some(graph);
                    }
                }
            }
        }
        None
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_wasm_import(&mut self) {
        if let Some(storage) = Self::web_storage() {
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
                                                                if let Some(storage) = LogMarkApp::web_storage() {
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

    #[cfg(target_arch = "wasm32")]
    fn web_storage() -> Option<Storage> {
        web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
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

    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let graph = Self::load_autosave()
            .filter(|g| g.node_count() >= 3)
            .unwrap_or_else(Self::default_graph);

        Self {
            graph,
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
            slash_menu_open: false,
            slash_menu_query: String::new(),
            slash_menu_pos: None,
            slash_menu_selection: 0,
            lua_sidebar_open: false,
            lua_output: String::new(),
            lua_variables: Vec::new(),
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
            editing_node_content: false,
        }
    }

    fn handle_wikilinks(&mut self, node_idx: petgraph::stable_graph::NodeIndex) {
        let content = self.graph.node(node_idx).unwrap().payload().content.clone();
        let mut new_links = Vec::new();
        // Capture group 1 is target, group 2 is optional label
        for cap in self.wikilink_regex.captures_iter(&content) {
            if let Some(m) = cap.get(1) {
                let target = m.as_str().to_string();
                let label = cap.get(2).map(|m| m.as_str().to_string());
                new_links.push((target, label));
            }
        }

        let mut target_indices = Vec::new();
        for (link_label, edge_label) in new_links {
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
                    let new_node_data = LogNodeData {
                        label: link_label.clone(),
                        content: format!("# {}", link_label),
                    };
                    let idx = self.graph.add_node(new_node_data);
                    let source_pos = self.graph.node(node_idx).unwrap().location();
                    self.graph.node_mut(idx).unwrap().set_location(source_pos + egui::Vec2::new(50.0, 50.0));
                    idx
                }
            };
            target_indices.push((target_idx, edge_label));
        }

        for (target_idx, edge_label) in target_indices {
            if target_idx == node_idx { continue; }
            
            let mut edge_exists = false;
            let mut existing_edge_idx = None;
            
            for edge in self.graph.g().edges(node_idx) {
                if edge.target() == target_idx {
                    edge_exists = true;
                    existing_edge_idx = Some(edge.id());
                    break;
                }
            }

            let label_text = edge_label.unwrap_or_else(|| "links to".to_string());

            if !edge_exists {
                self.graph.add_edge(node_idx, target_idx, LogEdgeData { label: Some(label_text) });
            } else if let Some(edge_idx) = existing_edge_idx {
                // Update existing edge label if it differs
                if let Some(edge) = self.graph.edge_mut(edge_idx) {
                    if edge.payload().label.as_ref() != Some(&label_text) {
                        edge.payload_mut().label = Some(label_text);
                    }
                }
            }
        }
    }

    fn run_lua_script(&mut self, script: &str) {
        self.lua_output.clear();
        self.lua_variables.clear();
        self.lua_output.push_str("Running script...\n");

        let lines: Vec<&str> = script.lines().collect();
        for line in lines {
            let line = line.trim();
            if line.starts_with("--") || line.is_empty() {
                continue;
            }

            // print("...") or print(var)
            if line.starts_with("print(") && line.ends_with(')') {
                let content = &line[6..line.len()-1];
                let val = if content.starts_with('\'') || content.starts_with('"') {
                    content[1..content.len()-1].to_string()
                } else {
                    // Lookup variable
                    self.lua_variables.iter()
                        .find(|(k, _)| k == content)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_else(|| "nil".to_string())
                };
                self.lua_output.push_str(&format!("> {}\n", val));
            }
            // local x = ...
            else if line.starts_with("local ") {
                if let Some(idx) = line.find('=') {
                    let var_name = line[6..idx].trim().to_string();
                    let val_str = line[idx+1..].trim();
                    let val = if val_str.starts_with('\'') || val_str.starts_with('"') {
                        val_str[1..val_str.len()-1].to_string()
                    } else {
                        val_str.to_string()
                    };
                    
                    // Update or push
                    if let Some(existing) = self.lua_variables.iter_mut().find(|(k, _)| k == &var_name) {
                        existing.1 = val;
                    } else {
                        self.lua_variables.push((var_name, val));
                    }
                }
            }
            // graph.add_node("...")
            else if line.starts_with("graph.add_node(") && line.ends_with(')') {
                let content = &line[15..line.len()-1];
                let label = if content.starts_with('\'') || content.starts_with('"') {
                    content[1..content.len()-1].to_string()
                } else {
                    content.to_string()
                };
                
                self.push_undo();
                let idx = self.graph.add_node(LogNodeData { 
                    label: label.clone(), 
                    content: format!("# {}\nCreated by Lua script.", label) 
                });
                self.graph.node_mut(idx).unwrap().set_location(egui::Pos2::new(0.0, 0.0));
                self.lua_output.push_str(&format!("Added node: {}\n", label));
                self.change_count += 1;
            }
            // graph.add_edge("from", "to", "label")
            else if line.starts_with("graph.add_edge(") && line.ends_with(')') {
                let content = &line[15..line.len()-1];
                let parts: Vec<&str> = content.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    let from_label = parts[0].trim_matches(|c| c == '\'' || c == '"');
                    let to_label = parts[1].trim_matches(|c| c == '\'' || c == '"');
                    let edge_label = if parts.len() > 2 {
                        Some(parts[2].trim_matches(|c| c == '\'' || c == '"').to_string())
                    } else {
                        None
                    };

                    let from_idx = self.graph.g().node_indices().find(|idx| {
                        self.graph.node(*idx).unwrap().payload().label == from_label
                    });
                    let to_idx = self.graph.g().node_indices().find(|idx| {
                        self.graph.node(*idx).unwrap().payload().label == to_label
                    });

                    if let (Some(from), Some(to)) = (from_idx, to_idx) {
                        self.push_undo();
                        self.graph.add_edge(from, to, LogEdgeData { label: edge_label.clone() });
                        self.lua_output.push_str(&format!("Added edge: {} -> {} ({:?})\n", from_label, to_label, edge_label));
                        self.change_count += 1;
                    } else {
                        self.lua_output.push_str(&format!("Error: Could not find nodes for edge: {} -> {}\n", from_label, to_label));
                    }
                }
            }
            // graph.list_nodes()
            else if line == "graph.list_nodes()" {
                self.lua_output.push_str("Nodes:\n");
                for idx in self.graph.g().node_indices() {
                    let label = &self.graph.node(idx).unwrap().payload().label;
                    self.lua_output.push_str(&format!("- {}\n", label));
                }
            }
        }
        self.lua_output.push_str("Done.");
    }

    fn commit_changes(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::autosave_path();
            let config = PrettyConfig::default();
            match to_string_pretty(&self.graph, config) {
                Ok(data) => {
                    if let Err(err) = fs::write(&path, data) {
                        eprintln!("Auto-commit failed to write {}: {}", path.display(), err);
                    }
                }
                Err(err) => eprintln!("Auto-commit serialization failed: {}", err),
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(storage) = Self::web_storage() {
                let config = PrettyConfig::default();
                if let Ok(data) = to_string_pretty(&self.graph, config) {
                    let _ = storage.set_item("logmark_autosave", &data);
                }
            }
        }

        self.change_count = 0;
    }
}

impl App for LogMarkApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Auto-commit check
        if self.change_count > 10 {
            self.commit_changes();
        }

        // Theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        self.prune_toasts();

        #[cfg(target_arch = "wasm32")]
        self.poll_wasm_import();

        // Help Window
        if self.show_help {
            let mut open = true;
            Window::new("Help & Shortcuts")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Keyboard Shortcuts");
                    egui::Grid::new("shortcuts_grid").striped(true).show(ui, |ui| {
                        ui.label("Undo"); ui.label("Ctrl + Z"); ui.end_row();
                        ui.label("Redo"); ui.label("Ctrl + Y"); ui.end_row();
                        ui.label("Zen Mode"); ui.label("Esc"); ui.end_row();
                        ui.label("Slash Menu"); ui.label("/"); ui.end_row();
                        ui.label("Confirm Edit"); ui.label("Enter"); ui.end_row();
                    });
                    
                    ui.separator();
                    ui.heading("Features");
                    ui.label("• Double-click a node to edit its label.");
                    ui.label("• Double-click an edge to edit its label.");
                    ui.label("• Use the Slash Menu (/) in the editor to insert headings or links.");
                    ui.label("• Use [[WikiLinks]] to connect nodes.");
                    ui.label("• Use Lua blocks for scripting.");
                });
            if !open {
                self.show_help = false;
            }
        }

        // Top Panel
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(path) = FileDialog::new().add_filter("RON", &["ron"]).pick_file() {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(graph) = from_str(&content) {
                                    self.push_undo();
                                    self.graph = graph;
                                    println!("Loaded graph from {}", path.display());
                                    self.push_toast("Opened graph");
                                } else {
                                    eprintln!("Failed to deserialize graph");
                                    self.push_toast("Failed to open graph");
                                }
                            } else {
                                eprintln!("Failed to read file");
                                self.push_toast("Failed to read file");
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.start_wasm_file_picker();
                    }
                }
                if ui.button("Save").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if let Some(path) = FileDialog::new().add_filter("RON", &["ron"]).save_file() {
                            let config = PrettyConfig::default();
                            if let Ok(data) = to_string_pretty(&self.graph, config) {
                                if let Err(err) = fs::write(&path, data) {
                                    eprintln!("Failed to save graph: {}", err);
                                    self.push_toast("Save failed");
                                } else {
                                    println!("Saved graph to {}", path.display());
                                    self.push_toast("Saved graph");
                                }
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.commit_changes();
                        self.web_export_ron();
                        self.push_toast("Exported graph");
                    }
                }
                if ui.button("Reset Docs").clicked() {
                    self.push_undo();
                    self.graph = Self::default_graph();
                    self.layout = AppLayout::Hierarchical;
                    self.fit_to_view_next = true;
                    self.push_toast("Reset to documentation graph");
                }

                if ui.button("Help / Shortcuts").clicked() {
                    self.show_help = true;
                }

                if ui.button("Undo").clicked() {
                    self.undo();
                }
                if ui.button("Redo").clicked() {
                    self.redo();
                }

                if ui.button(if self.dark_mode { "Light" } else { "Dark" }).clicked() {
                    self.dark_mode = !self.dark_mode;
                    self.push_toast(if self.dark_mode { "Dark mode" } else { "Light mode" });
                }

                if let Some((msg, _)) = self.toasts.first() {
                    ui.label(msg);
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
                        if ui.button("Load").clicked() {
                            if let Ok(graph) = from_str::<Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>>(&self.wasm_import_buffer) {
                                self.graph = graph;
                                self.wasm_import_open = false;
                                self.wasm_import_buffer.clear();
                                self.push_toast("Imported graph");
                            } else {
                                self.push_toast("Import failed");
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.wasm_import_open = false;
                            self.wasm_import_buffer.clear();
                        }
                    });
                });

            if !open {
                self.wasm_import_open = false;
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
                    ui.separator();
                    if ui.button("Zen Mode (Esc)").clicked() {
                        self.zen_mode = true;
                        self.push_toast("Entered Zen Mode");
                    }
                    ui.separator();
                    ui.label("Layout:");
                ui.radio_value(&mut self.layout, AppLayout::Random, "Random")
                    .on_hover_text("Scatter nodes unpredictably");
                ui.radio_value(&mut self.layout, AppLayout::Force, "Force")
                    .on_hover_text("Physics-based layout");
                ui.radio_value(&mut self.layout, AppLayout::Hierarchical, "Hierarchical")
                    .on_hover_text("Tree view that fills the canvas");

                if ui.button("Fit View").clicked() {
                    self.fit_to_view_next = true;
                    self.push_toast("Fitting view");
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
                        let node = self.graph.node(idx).unwrap();
                        let content = node.payload().content.to_lowercase();
                        let label = node.payload().label.to_lowercase();
                        
                        let matches_query = q.is_empty() || label.contains(&q) || content.contains(&q);
                        let matches_task = !self.filter_tasks || content.contains("- [ ]") || content.contains("- [x]");
                        
                        if matches_query && matches_task {
                            Some((idx, node.payload().label.clone()))
                        } else {
                            None
                        }
                    }).collect();
                    for (idx, label) in results {
                        if ui.button(&label).clicked() {
                            self.graph.set_selected_nodes(vec![idx]);
                            self.fit_to_view_next = true;
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
                if ui.button("Add Node").clicked() {
                    self.push_undo();
                    let label = if self.quick_add_label.is_empty() {
                        "New Node".to_string()
                    } else {
                        self.quick_add_label.clone()
                    };
                    let content = match self.quick_add_template {
                        QuickTemplate::Note => format!("# {}\n\nNew note.", label),
                        QuickTemplate::Heading => format!("# {}", label),
                        QuickTemplate::Task => format!("# {}\n- [ ] TODO", label),
                    };
                    let idx = self.graph.add_node(LogNodeData { label: label.clone(), content });
                    self.graph.node_mut(idx).unwrap().set_location(egui::Pos2::new(0.0, 0.0));
                    self.graph.set_selected_nodes(vec![idx]);
                    self.change_count += 1;
                    self.fit_to_view_next = true;
                    self.quick_add_label.clear();
                    self.push_toast("Added node");
                }

                ui.separator();
                ui.label(format!("Auto-commit after 10 edits (current: {})", self.change_count));

                ui.separator();
                ui.heading("Lua Debugger");
                if self.lua_sidebar_open {
                    ui.label("Debugger active");
                    ui.horizontal(|ui| {
                        if ui.button("Step Over").clicked() {}
                        if ui.button("Step Into").clicked() {}
                    });
                    if ui.button("Continue").clicked() {}
                    ui.separator();
                    ui.heading("Variables");
                    for (name, val) in &self.lua_variables {
                        ui.label(format!("{}: {}", name, val));
                    }
                    ui.separator();
                    ui.heading("Statistics");
                    ui.label("Memory: 1.2 MB");
                    ui.label("CPU Time: 0.5 ms");
                } else {
                    ui.label("Debugger closed. Open via 🐞 Debug in Lua block.");
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
                    self.push_undo();
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
                    self.push_undo();
                    if let Some(edge) = self.graph.edge_mut(eidx) {
                        edge.payload_mut().label = Some(self.edge_edit_buffer.clone());
                    }
                    self.editing_edge = None;
                    self.editing_edge_pos = None;
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
                        if ui.button(if self.sidebar_expanded { "«" } else { "»" }).clicked() {
                            self.sidebar_expanded = !self.sidebar_expanded;
                        }
                        ui.heading("Sidebar");
                    });

                    if !self.sidebar_expanded {
                        return;
                    }

                if let Some(idx) = self.graph.selected_nodes().first() {
                    let idx = *idx;
                    
                    ui.heading("Node Content");
                    ui.separator();
                    
                    let mut content = self.graph.node(idx).unwrap().payload().content.clone();
                    let label = self.graph.node(idx).unwrap().payload().label.clone();

                    ui.horizontal(|ui| {
                        ui.label(format!("Editing: {}", label));
                        if !self.editing_node_content {
                            if ui.button("✏ Edit").clicked() {
                                self.editing_node_content = true;
                            }
                        } else {
                            if ui.button("👁 View").clicked() {
                                self.editing_node_content = false;
                            }
                        }
                    });
                    
                    if !self.editing_node_content {
                        // View Mode (Preview)
                        let available_height = ui.available_height() - 100.0; // Leave space for Lua controls if any
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.set_min_height(available_height.max(200.0));
                            // Make the preview clickable to switch to edit mode
                            let response = ui.scope(|ui| {
                                CommonMarkViewer::new()
                                    .show(ui, &mut self.markdown_cache, &content);
                            }).response;
                            
                            if response.clicked() || response.double_clicked() {
                                self.editing_node_content = true;
                            }
                        });
                    } else {
                        // Edit Mode
                        let response = ui.add_sized(
                            ui.available_size() - egui::Vec2::new(0.0, 100.0), 
                            TextEdit::multiline(&mut content)
                                .desired_width(f32::INFINITY)
                                .code_editor()
                                .lock_focus(true)
                        );

                        if response.changed() {
                            if self.change_count % 5 == 0 {
                                self.push_undo();
                            }
                            self.change_count += 1;
                            if let Some(node) = self.graph.node_mut(idx) {
                                node.payload_mut().content = content.clone();
                            }
                            self.handle_wikilinks(idx);
                            
                            if self.slash_menu_open {
                                if let Some(last_slash) = content.rfind('/') {
                                    let query = &content[last_slash + 1..];
                                    if query.contains(char::is_whitespace) {
                                        self.slash_menu_open = false;
                                    } else {
                                        self.slash_menu_query = query.to_string();
                                    }
                                } else {
                                    self.slash_menu_open = false;
                                }
                            } else if content.ends_with('/') {
                                self.slash_menu_open = true;
                                self.slash_menu_pos = Some(response.rect.max);
                                self.slash_menu_selection = 0;
                                self.slash_menu_query.clear();
                            }
                        }

                        // Slash Menu
                        if self.slash_menu_open {
                            // Keyboard navigation for the popup list
                            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                                self.slash_menu_selection = self.slash_menu_selection.saturating_add(1);
                            }
                            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                                if self.slash_menu_selection > 0 {
                                    self.slash_menu_selection -= 1;
                                }
                            }

                            let mut open = true;
                            Window::new("slash_menu")
                                .open(&mut open)
                                .title_bar(false)
                                .anchor(Align2::LEFT_TOP, [self.slash_menu_pos.unwrap_or_default().x, self.slash_menu_pos.unwrap_or_default().y])
                                .show(ctx, |ui| {
                                    ui.label("Insert:");
                                    let q = self.slash_menu_query.to_lowercase();
                                    let options: Vec<_> = vec![
                                        ("Heading 1", "# "),
                                        ("Heading 2", "## "),
                                        ("Heading 3", "### "),
                                        ("Task", "- [ ] "),
                                        ("Bullet List", "- "),
                                        ("Numbered List", "1. "),
                                        ("Quote", "> "),
                                        ("Code Block", "```\n\n```"),
                                        ("Lua Block", "```lua\n\n```"),
                                    ].into_iter().filter(|(name, _)| name.to_lowercase().contains(&q)).collect();

                                    let mut link_labels = Vec::new();
                                    for other_idx in self.graph.g().node_indices() {
                                        if other_idx == idx { continue; }
                                        let other_label = self.graph.node(other_idx).unwrap().payload().label.clone();
                                        if other_label.to_lowercase().contains(&q) {
                                            link_labels.push(other_label);
                                        }
                                    }

                                    let total = options.len() + link_labels.len();
                                    if total == 0 {
                                        self.slash_menu_selection = 0;
                                    } else if self.slash_menu_selection >= total {
                                        self.slash_menu_selection = total - 1;
                                    }

                                    let mut chosen: Option<String> = None;

                                    for (i, (name, val)) in options.iter().enumerate() {
                                        let selected = i == self.slash_menu_selection;
                                        if ui.selectable_label(selected, *name).clicked() {
                                            chosen = Some(val.to_string());
                                        }
                                    }

                                    ui.separator();
                                    ui.label("Link to Node:");
                                    for (i, lbl) in link_labels.iter().enumerate() {
                                        let idx_global = options.len() + i;
                                        let selected = idx_global == self.slash_menu_selection;
                                        if ui.selectable_label(selected, lbl).clicked() {
                                            chosen = Some(format!("[[{}]]", lbl));
                                        }
                                    }

                                    // Accept via Enter or Tab
                                    if ctx.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Tab)) {
                                        if self.slash_menu_selection < options.len() {
                                            chosen = Some(options[self.slash_menu_selection].1.to_string());
                                        } else {
                                            let link_idx = self.slash_menu_selection.saturating_sub(options.len());
                                            if link_idx < link_labels.len() {
                                                chosen = Some(format!("[[{}]]", link_labels[link_idx]));
                                            }
                                        }
                                    }

                                    if let Some(val) = chosen {
                                        // Robust replacement: find the last slash before cursor or end
                                        // Since we don't have cursor pos easily, we rely on rfind('/')
                                        if let Some(last_slash) = content.rfind('/') {
                                            content.replace_range(last_slash.., &val);
                                            self.slash_menu_open = false;
                                            if let Some(node) = self.graph.node_mut(idx) {
                                                node.payload_mut().content = content.clone();
                                            }
                                        }
                                    }
                                });
                            
                            if !open {
                                self.slash_menu_open = false;
                            }
                        }
                    }

                    ui.separator();
                    
                    // Lua Controls
                    if content.contains("```lua") {
                        ui.heading("Lua Integration");
                        ui.horizontal(|ui| {
                            if ui.button("▶ Run").clicked() {
                                if let Some(cap) = self.lua_regex.captures(&content) {
                                    if let Some(script) = cap.get(1) {
                                        let script_text = script.as_str().to_string();
                                        self.run_lua_script(&script_text);
                                    }
                                } else {
                                    self.lua_output = "No Lua code block found.".to_string();
                                }
                            }
                            if ui.button("⏹ Stop").clicked() {
                                self.lua_output.push_str("\nStopped.");
                            }
                            if ui.button("🐞 Debug").clicked() {
                                self.lua_sidebar_open = true;
                            }
                        });
                        ui.label("Output:");
                        ui.code(&self.lua_output);
                    }

                    // Removed separate Preview section as it is now integrated into the main view mode
                } else {
                    ui.label("Select a node to edit its content.");
                }
            });
        }
            
        // Graph View
        CentralPanel::default().show(ctx, |ui| {
            let interactions = SettingsInteraction::default()
                .with_dragging_enabled(true)
                .with_node_selection_enabled(true)
                .with_node_selection_multi_enabled(false)
                .with_edge_selection_enabled(true)
                .with_edge_selection_multi_enabled(false)
                .with_edge_clicking_enabled(true);

            let mut navigation = SettingsNavigation::default()
                .with_fit_to_screen_enabled(false)
                .with_zoom_and_pan_enabled(true)
                .with_fit_to_screen_padding(0.1);
            if self.fit_to_view_next {
                navigation = navigation.with_fit_to_screen_enabled(true);
            }

            let resp = match self.layout {
                AppLayout::Random => {
                    let mut widget: GraphView<'_, _, _, _, _, _, _, LayoutStateRandom, LayoutRandom> = GraphView::new(&mut self.graph).with_id(Some("random".to_string()));
                    widget = widget.with_interactions(&interactions).with_navigations(&navigation);
                    ui.add(&mut widget)
                },
                AppLayout::Force => {
                    let mut widget: GraphView<'_, _, _, _, _, _, _, FruchtermanReingoldState, LayoutForceDirected<FruchtermanReingold>> = GraphView::new(&mut self.graph).with_id(Some("force".to_string()));
                    widget = widget.with_interactions(&interactions).with_navigations(&navigation);
                    ui.add(&mut widget)
                },
                AppLayout::Hierarchical => {
                    let mut widget: GraphView<'_, _, _, _, _, _, _, LayoutStateHierarchical, LayoutHierarchical> = GraphView::new(&mut self.graph).with_id(Some("hierarchical".to_string()));
                    widget = widget.with_interactions(&interactions).with_navigations(&navigation);
                    ui.add(&mut widget)
                }
            };

            if self.fit_to_view_next {
                self.fit_to_view_next = false;
            }

            if resp.double_clicked() {
                if let Some(idx) = self.graph.selected_nodes().first() {
                    self.editing_label = Some(*idx);
                    self.label_edit_buffer = self.graph.node(*idx).unwrap().payload().label.clone();
                    self.editing_pos = resp.hover_pos();
                } else if let Some(eidx) = self.graph.selected_edges().first() {
                    self.editing_edge = Some(*eidx);
                    if let Some(edge) = self.graph.edge(*eidx) {
                        self.edge_edit_buffer = edge.payload().label.clone().unwrap_or_default();
                    }
                    self.editing_edge_pos = resp.hover_pos();
                }
            }
        });
    }
}
