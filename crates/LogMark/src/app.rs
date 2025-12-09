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
    Graph, GraphView, SettingsInteraction, SettingsNavigation, MetadataFrame,
    LayoutStateRandom, LayoutRandom,
    LayoutForceDirected, FruchtermanReingold, FruchtermanReingoldState,
    LayoutHierarchical, LayoutStateHierarchical,
};
use petgraph::{stable_graph::StableGraph, Directed};
use petgraph::visit::EdgeRef;
use petgraph::visit::IntoEdgeReferences;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_code_editor::{CodeEditor, ColorTheme, Completer};
use regex::Regex;
use ron::de::from_str;
use ron::ser::{to_string_pretty, PrettyConfig};

use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};
use crate::parser::MarkdownParser;
use crate::syntax;

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
    
    // Parser
    parser: MarkdownParser,
    completer: Completer,

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
    #[allow(dead_code)]
    editor_mode: EditorMode,
    
    // Hybrid Editor State
    #[allow(dead_code)]
    editing_block_idx: Option<usize>,
    
    // Sidebar State
    sidebar_tab: SidebarTab,
    
    // Slash Menu State
    slash_menu_items: Vec<(&'static str, &'static str, &'static str)>, // (icon, label, template)
    content_edit_buffer: String,
    cursor_position: usize,
    last_edited_node: Option<petgraph::stable_graph::NodeIndex>, // Track which node the buffer belongs to
    
    // App-level Tab and Visualization (like code-analyzer-web)
    current_tab: AppTab,
    visualization_mode: VisualizationMode,
    
    // 3D View Settings
    rotation_x: f32,
    rotation_y: f32,
    rotation_z: f32,
    auto_rotate: bool,
    rotation_speed: f32,
    perspective_strength: f32,
    show_3d_settings: bool,
    
    // Entity Relationship Cardinality
    show_cardinality: bool,
}

/// Main app tabs (like code-analyzer-web)
#[derive(PartialEq, Clone, Copy)]
enum AppTab {
    Graph,
    Preview,
}

/// Visualization mode for the graph
#[derive(PartialEq, Clone, Copy)]
enum VisualizationMode {
    TwoD,
    ThreeD,
}

impl VisualizationMode {
    fn label(&self) -> &str {
        match self {
            VisualizationMode::TwoD => "2D View",
            VisualizationMode::ThreeD => "3D View",
        }
    }
}

/// Helper function for 3D to 2D projection with all rotation axes
#[allow(dead_code)]
fn project_3d_to_2d(
    pos: egui::Pos2,
    pivot: egui::Pos2,
    z: f32,
    rotation_x: f32,
    rotation_y: f32,
    rotation_z: f32,
    perspective_strength: f32,
) -> egui::Pos2 {
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
    let projected_local = egui::Pos2::new(x3 * scale, y2 * scale);
    pivot + projected_local.to_vec2()
}

#[derive(PartialEq, Clone, Copy)]
enum SidebarTab {
    Edit,
    Preview,
    Lua,
}

#[allow(dead_code)]
#[derive(PartialEq, Clone, Copy)]
enum EditorMode {
    View,
    Edit,
    Split,
    Hybrid, // Logseq-style
}

impl LogMarkApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn autosave_path() -> PathBuf {
        PathBuf::from("logmark_autosave.ron")
    }
    
    #[cfg(not(target_arch = "wasm32"))]
    fn default_graph_path() -> PathBuf {
        // Try to find default_graph.ron in several locations
        let paths = [
            PathBuf::from("assets/default_graph.ron"),
            PathBuf::from("default_graph.ron"),
            PathBuf::from("../assets/default_graph.ron"),
        ];
        for p in paths {
            if p.exists() {
                return p;
            }
        }
        PathBuf::from("assets/default_graph.ron")
    }

    fn default_graph() -> Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge> {
        // First, try to load from external file
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::default_graph_path();
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(graph) = from_str(&content) {
                        println!("Loaded default graph from {}", path.display());
                        return graph;
                    } else {
                        eprintln!("Failed to parse default_graph.ron, using hardcoded default");
                    }
                }
            }
        }
        
        // Fall back to hardcoded default graph
        Self::hardcoded_default_graph()
    }
    
    fn hardcoded_default_graph() -> Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge> {
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
            parser: MarkdownParser::new(),
            completer: Completer::new_with_syntax(&syntax::markdown()).with_user_words(),
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
            editor_mode: EditorMode::Hybrid,
            editing_block_idx: None,
            sidebar_tab: SidebarTab::Edit,
            // Slash menu items: (icon, label, markdown template with cursor position marked by |)
            slash_menu_items: vec![
                // Formatting - headings have cursor at end so user types right after
                ("#", "Heading 1", "# |"),
                ("##", "Heading 2", "## |"),
                ("###", "Heading 3", "### |"),
                ("•", "Bullet List", "- |"),
                ("☐", "Task", "- [ ] |"),
                (">", "Quote", "> |"),
                ("`", "Code Inline", "`|`"),
                ("```", "Code Block", "```\n|\n```"),
                ("🔗", "Wiki Link", "[[|]]"),
                ("🌐", "External Link", "[|](https://)"),
                ("---", "Divider", "---\n|"),
                ("**", "Bold", "**|**"),
                ("*", "Italic", "*|*"),
                ("~~", "Strikethrough", "~~|~~"),
                // Lua Scripts
                ("📝", "Lua Block", "```lua\n|\n```"),
                ("🔧", "Lua: Add Node", "```lua\ngraph.add_node('|')\n```"),
                ("🔗", "Lua: Add Edge", "```lua\ngraph.add_edge('from', 'to', '|')\n```"),
                ("📊", "Lua: List Nodes", "```lua\ngraph.list_nodes()\n|\n```"),
                ("🔍", "Lua: Find Path", "```lua\ngraph.find_path('|', '')\n```"),
                ("📋", "Lua: Get Neighbors", "```lua\ngraph.get_neighbors('|')\n```"),
                ("✏️", "Lua: Set Content", "```lua\ngraph.set_node_content('|', 'content')\n```"),
            ],
            content_edit_buffer: String::new(),
            cursor_position: 0,
            last_edited_node: None,
            // App-level Tab and Visualization
            current_tab: AppTab::Graph,
            visualization_mode: VisualizationMode::TwoD,
            // 3D View Settings
            rotation_x: 30.0,
            rotation_y: 45.0,
            rotation_z: 0.0,
            auto_rotate: false,
            rotation_speed: 0.5,
            perspective_strength: 0.001,
            show_3d_settings: false,
            // Entity Relationship Cardinality
            show_cardinality: false,
        }
    }

    fn handle_wikilinks(&mut self, node_idx: petgraph::stable_graph::NodeIndex) {
        let content = self.graph.node(node_idx).unwrap().payload().content.clone();
        
        // 1. Parse current wikilinks from content using Tree-sitter to exclude code blocks
        let mut current_links = std::collections::HashMap::new();
        
        // Get safe ranges (not code blocks)
        let safe_ranges = self.parser.get_safe_ranges(&content);
        
        for range in safe_ranges {
            let slice = &content[range];
            for cap in self.wikilink_regex.captures_iter(slice) {
                if let Some(m) = cap.get(1) {
                    let target_label = m.as_str().to_string();
                    let edge_label = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| "links to".to_string());
                    current_links.insert(target_label, edge_label);
                }
            }
        }

        // 2. Identify existing edges from this node
        let mut edges_to_remove = Vec::new();
        let mut existing_targets = std::collections::HashMap::new(); // target_idx -> edge_idx

        for edge in self.graph.g().edges(node_idx) {
            let target_idx = edge.target();
            if target_idx == node_idx { continue; }
            existing_targets.insert(target_idx, edge.id());
        }

        // 3. Process edges to remove (those not in current_links)
        for (target_idx, edge_idx) in &existing_targets {
            if let Some(target_node) = self.graph.node(*target_idx) {
                let target_label = &target_node.payload().label;
                if !current_links.contains_key(target_label) {
                    edges_to_remove.push((*edge_idx, *target_idx));
                }
            }
        }

        // Remove edges and cleanup orphans
        for (edge_idx, target_idx) in edges_to_remove {
            self.graph.g_mut().remove_edge(edge_idx);
            
            // Check if target node is now an orphan and should be deleted
            let is_orphan = self.graph.g().edges_directed(target_idx, petgraph::Direction::Incoming).count() == 0;
            if is_orphan {
                if let Some(node) = self.graph.node(target_idx) {
                    let default_content = format!("# {}", node.payload().label);
                    if node.payload().content == default_content {
                        self.graph.remove_node(target_idx);
                    }
                }
            }
        }

        // 4. Process current links (add or update edges)
        for (target_label, edge_label) in current_links {
            let mut target_idx = None;
            for idx in self.graph.g().node_indices() {
                if let Some(node) = self.graph.node(idx) {
                    if node.payload().label == target_label {
                        target_idx = Some(idx);
                        break;
                    }
                }
            }

            // Create node if it doesn't exist (only when fully typed as a valid wikilink)
            let target_idx = match target_idx {
                Some(idx) => idx,
                None => {
                    let new_node_data = LogNodeData {
                        label: target_label.clone(),
                        content: format!("# {}", target_label),
                    };
                    let idx = self.graph.add_node(new_node_data);
                    let source_pos = self.graph.node(node_idx).unwrap().location();
                    self.graph.node_mut(idx).unwrap().set_location(source_pos + egui::Vec2::new(50.0, 50.0));
                    idx
                }
            };

            if target_idx == node_idx { continue; }

            if let Some(&edge_idx) = existing_targets.get(&target_idx) {
                if let Some(edge) = self.graph.edge_mut(edge_idx) {
                    if edge.payload().label.as_ref() != Some(&edge_label) {
                        edge.payload_mut().label = Some(edge_label);
                    }
                }
            } else {
                self.graph.add_edge(node_idx, target_idx, LogEdgeData { label: Some(edge_label) });
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
            // graph.list_edges()
            else if line == "graph.list_edges()" {
                self.lua_output.push_str("Edges:\n");
                for edge_idx in self.graph.g().edge_indices() {
                    if let Some(edge) = self.graph.edge(edge_idx) {
                        let (source, target) = self.graph.g().edge_endpoints(edge_idx).unwrap();
                        let from = &self.graph.node(source).unwrap().payload().label;
                        let to = &self.graph.node(target).unwrap().payload().label;
                        let label = edge.payload().label.as_deref().unwrap_or("");
                        self.lua_output.push_str(&format!("- {} -> {} ({})\n", from, to, label));
                    }
                }
            }
            // graph.node_count()
            else if line == "graph.node_count()" {
                let count = self.graph.g().node_count();
                self.lua_output.push_str(&format!("> {}\n", count));
            }
            // graph.edge_count()
            else if line == "graph.edge_count()" {
                let count = self.graph.g().edge_count();
                self.lua_output.push_str(&format!("> {}\n", count));
            }
            // graph.get_node_content("label")
            else if line.starts_with("graph.get_node_content(") && line.ends_with(')') {
                let content = &line[23..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = self.graph.g().node_indices().find(|idx| {
                    self.graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    let content = &self.graph.node(idx).unwrap().payload().content;
                    self.lua_output.push_str(&format!("> {}\n", content));
                } else {
                    self.lua_output.push_str(&format!("Error: Node not found: {}\n", label));
                }
            }
            // graph.set_node_content("label", "content")
            else if line.starts_with("graph.set_node_content(") && line.ends_with(')') {
                let args = &line[23..line.len()-1];
                let parts: Vec<&str> = args.splitn(2, ',').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    let label = parts[0].trim_matches(|c| c == '\'' || c == '"');
                    let new_content = parts[1].trim_matches(|c| c == '\'' || c == '"');
                    let node = self.graph.g().node_indices().find(|idx| {
                        self.graph.node(*idx).unwrap().payload().label == label
                    });
                    if let Some(idx) = node {
                        self.push_undo();
                        self.graph.node_mut(idx).unwrap().payload_mut().content = new_content.to_string();
                        self.lua_output.push_str(&format!("Set content for: {}\n", label));
                        self.change_count += 1;
                    } else {
                        self.lua_output.push_str(&format!("Error: Node not found: {}\n", label));
                    }
                }
            }
            // graph.remove_node("label")
            else if line.starts_with("graph.remove_node(") && line.ends_with(')') {
                let content = &line[18..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = self.graph.g().node_indices().find(|idx| {
                    self.graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    self.push_undo();
                    self.graph.remove_node(idx);
                    self.lua_output.push_str(&format!("Removed node: {}\n", label));
                    self.change_count += 1;
                } else {
                    self.lua_output.push_str(&format!("Error: Node not found: {}\n", label));
                }
            }
            // graph.get_neighbors("label")
            else if line.starts_with("graph.get_neighbors(") && line.ends_with(')') {
                let content = &line[20..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = self.graph.g().node_indices().find(|idx| {
                    self.graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    self.lua_output.push_str(&format!("Neighbors of {}:\n", label));
                    for neighbor in self.graph.g().neighbors(idx) {
                        let nlabel = &self.graph.node(neighbor).unwrap().payload().label;
                        self.lua_output.push_str(&format!("- {}\n", nlabel));
                    }
                } else {
                    self.lua_output.push_str(&format!("Error: Node not found: {}\n", label));
                }
            }
            // graph.find_path("from", "to")
            else if line.starts_with("graph.find_path(") && line.ends_with(')') {
                let args = &line[16..line.len()-1];
                let parts: Vec<&str> = args.split(',').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    let from_label = parts[0].trim_matches(|c| c == '\'' || c == '"');
                    let to_label = parts[1].trim_matches(|c| c == '\'' || c == '"');
                    let from_idx = self.graph.g().node_indices().find(|idx| {
                        self.graph.node(*idx).unwrap().payload().label == from_label
                    });
                    let to_idx = self.graph.g().node_indices().find(|idx| {
                        self.graph.node(*idx).unwrap().payload().label == to_label
                    });
                    if let (Some(from), Some(to)) = (from_idx, to_idx) {
                        // Simple BFS path finding
                        use std::collections::{VecDeque, HashSet, HashMap};
                        let mut queue: VecDeque<petgraph::stable_graph::NodeIndex> = VecDeque::new();
                        let mut visited: HashSet<petgraph::stable_graph::NodeIndex> = HashSet::new();
                        let mut parents: HashMap<petgraph::stable_graph::NodeIndex, petgraph::stable_graph::NodeIndex> = HashMap::new();
                        queue.push_back(from);
                        visited.insert(from);
                        let mut found = false;
                        while let Some(current) = queue.pop_front() {
                            if current == to {
                                found = true;
                                break;
                            }
                            for neighbor in self.graph.g().neighbors(current) {
                                if !visited.contains(&neighbor) {
                                    visited.insert(neighbor);
                                    parents.insert(neighbor, current);
                                    queue.push_back(neighbor);
                                }
                            }
                        }
                        if found {
                            let mut path = Vec::new();
                            let mut current = to;
                            while current != from {
                                path.push(self.graph.node(current).unwrap().payload().label.clone());
                                current = parents[&current];
                            }
                            path.push(self.graph.node(from).unwrap().payload().label.clone());
                            path.reverse();
                            self.lua_output.push_str(&format!("Path: {}\n", path.join(" -> ")));
                        } else {
                            self.lua_output.push_str("No path found.\n");
                        }
                    } else {
                        self.lua_output.push_str("Error: One or both nodes not found.\n");
                    }
                }
            }
            // graph.get_selected()
            else if line == "graph.get_selected()" {
                if let Some(idx) = self.graph.selected_nodes().first() {
                    let label = &self.graph.node(*idx).unwrap().payload().label;
                    self.lua_output.push_str(&format!("> {}\n", label));
                } else {
                    self.lua_output.push_str("> nil\n");
                }
            }
            // graph.select("label")
            else if line.starts_with("graph.select(") && line.ends_with(')') {
                let content = &line[13..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = self.graph.g().node_indices().find(|idx| {
                    self.graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    self.graph.node_mut(idx).unwrap().set_selected(true);
                    self.lua_output.push_str(&format!("Selected: {}\n", label));
                } else {
                    self.lua_output.push_str(&format!("Error: Node not found: {}\n", label));
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
    
    /// Calculate the cardinality of an edge based on the in/out degree of connected nodes
    /// Returns a string like "1:1", "1:N", or "N:N"
    fn calculate_edge_cardinality(&self, source: petgraph::stable_graph::NodeIndex, target: petgraph::stable_graph::NodeIndex) -> String {
        use petgraph::Direction;
        
        // Count outgoing edges from source to determine the "one" or "many" on source side
        let source_out_degree = self.graph.g().edges_directed(source, Direction::Outgoing).count();
        
        // Count incoming edges to target to determine the "one" or "many" on target side
        let target_in_degree = self.graph.g().edges_directed(target, Direction::Incoming).count();
        
        let source_card = if source_out_degree <= 1 { "1" } else { "N" };
        let target_card = if target_in_degree <= 1 { "1" } else { "N" };
        
        format!("{}:{}", source_card, target_card)
    }
    
    /// Insert the selected slash menu template at the current cursor position
    fn insert_slash_template(&mut self, node_idx: petgraph::stable_graph::NodeIndex) {
        // Get the filtered items based on current query
        let filtered_items: Vec<_> = self.slash_menu_items.iter()
            .filter(|(_, item_label, _)| {
                self.slash_menu_query.is_empty() || 
                item_label.to_lowercase().contains(&self.slash_menu_query.to_lowercase())
            })
            .collect();
        
        if let Some((_, _, template)) = filtered_items.get(self.slash_menu_selection) {
            // cursor_position is where cursor was right after typing '/'
            // The user may have typed more characters after '/' which become the query
            // We need to remove '/' + query characters from the buffer
            let chars: Vec<char> = self.content_edit_buffer.chars().collect();
            let slash_pos = self.cursor_position.saturating_sub(1); // Position of '/'
            
            // Find how many characters to remove: '/' + any query typed after it
            // Look for the actual end of the /query in the buffer
            let query_len = self.slash_menu_query.chars().count();
            let remove_end = (slash_pos + 1 + query_len).min(chars.len());
            
            let before_slash: String = chars.iter().take(slash_pos).collect();
            let after_query: String = chars.iter().skip(remove_end).collect();
            
            // Check if / was at start of line (for conditional behavior)
            let _is_at_line_start = slash_pos == 0 || 
                (slash_pos > 0 && chars.get(slash_pos - 1) == Some(&'\n'));
            
            // Find where the cursor should be placed (marked by '|' in template)
            let template_str = *template;
            let cursor_marker_pos = template_str.find('|').unwrap_or(template_str.len());
            let template_before_cursor: String = template_str.chars().take(cursor_marker_pos).collect();
            let template_after_cursor: String = template_str.chars().skip(cursor_marker_pos + 1).collect();
            
            // Build new content
            // If at start of line: replace /query with template
            // If mid-line: just insert template (replace /query with template)
            let new_content = format!("{}{}{}{}", before_slash, template_before_cursor, template_after_cursor, after_query);
            self.content_edit_buffer = new_content;
            
            // Calculate new cursor position
            self.cursor_position = before_slash.chars().count() + template_before_cursor.chars().count();
            
            // Save to node
            if let Some(node) = self.graph.node_mut(node_idx) {
                node.payload_mut().content = self.content_edit_buffer.clone();
            }
            self.change_count += 1;
            self.handle_wikilinks(node_idx);
        }
        
        // Close the menu
        self.slash_menu_open = false;
        self.slash_menu_query.clear();
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

        // Top Panel (like code-analyzer-web menu bar)
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
                        ui.selectable_value(&mut self.visualization_mode, VisualizationMode::TwoD, "2D View");
                        ui.selectable_value(&mut self.visualization_mode, VisualizationMode::ThreeD, "3D View");
                    });
                
                if self.visualization_mode == VisualizationMode::ThreeD {
                    if ui.button("⚙ 3D Settings").clicked() {
                        self.show_3d_settings = !self.show_3d_settings;
                    }
                }
                
                ui.separator();
                
                // File operations
                if ui.button("📄 New").clicked() {
                    self.push_undo();
                    // Create a minimal new graph with one starting node
                    let mut g = StableGraph::new();
                    let start = g.add_node(LogNodeData {
                        label: "Start Here".to_string(),
                        content: "# Start Here\n\nThis is your new graph. Double-click to edit this node, or use the Quick Add panel on the left to create more nodes.\n\nTry typing `/` in the edit tab to see the slash menu!".to_string(),
                    });
                    let mut graph = Graph::from(&g);
                    if let Some(node) = graph.node_mut(start) {
                        node.set_location(egui::Pos2::new(0.0, 0.0));
                    }
                    self.graph = graph;
                    self.graph.set_selected_nodes(vec![]); // Clear selection
                    self.layout = AppLayout::Hierarchical;
                    self.fit_to_view_next = true;
                    self.content_edit_buffer.clear(); // Clear edit buffer
                    self.last_edited_node = None; // Reset tracked node
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
                                    self.last_edited_node = None; // Reset tracked node
                                    self.content_edit_buffer.clear();
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
                if ui.button("💾 Save").clicked() {
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
                
                ui.separator();
                
                if ui.button("📚 Reset Docs").clicked() {
                    self.push_undo();
                    self.graph = Self::default_graph();
                    self.layout = AppLayout::Hierarchical;
                    self.fit_to_view_next = true;
                    self.last_edited_node = None; // Reset tracked node
                    self.content_edit_buffer.clear();
                    self.push_toast("Reset to documentation graph");
                }

                if ui.button("❓ Help").clicked() {
                    self.show_help = true;
                }

                if ui.button("↩ Undo").clicked() {
                    self.undo();
                }
                if ui.button("↪ Redo").clicked() {
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

        // 3D Visualization Settings Window
        if self.show_3d_settings {
            let mut open = true;
            Window::new("3D Visualization Settings")
                .open(&mut open)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Rotation");
                    ui.add(egui::Slider::new(&mut self.rotation_x, -180.0..=180.0).text("X Axis"));
                    ui.add(egui::Slider::new(&mut self.rotation_y, -180.0..=180.0).text("Y Axis"));
                    ui.add(egui::Slider::new(&mut self.rotation_z, -180.0..=180.0).text("Z Axis"));
                    
                    ui.separator();
                    
                    ui.checkbox(&mut self.auto_rotate, "Auto Rotate");
                    if self.auto_rotate {
                        ui.add(egui::Slider::new(&mut self.rotation_speed, 0.1..=5.0).text("Rotation Speed"));
                    }
                    
                    ui.separator();
                    
                    ui.heading("Perspective");
                    ui.add(egui::Slider::new(&mut self.perspective_strength, 0.0..=0.01).text("Strength"));
                    
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("↺ Reset").clicked() {
                            self.rotation_x = 30.0;
                            self.rotation_y = 45.0;
                            self.rotation_z = 0.0;
                            self.auto_rotate = false;
                            self.rotation_speed = 0.5;
                            self.perspective_strength = 0.001;
                        }
                    });
                });
            if !open {
                self.show_3d_settings = false;
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
                            let source = edge.source();
                            let target = edge.target();
                            
                            let source_out = self.graph.g().edges(source).count();
                            let target_in = self.graph.g().edges_directed(target, petgraph::Direction::Incoming).count();
                            
                            if source_out == 1 && target_in == 1 {
                                one_to_one += 1;
                            } else if source_out == 1 || target_in == 1 {
                                one_to_many += 1;
                            } else {
                                many_to_many += 1;
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
                    
                    // Tab Bar
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Edit, "✏ Edit");
                        ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Preview, "👁 Preview");
                        ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Lua, "🔧 Lua");
                    });
                    ui.separator();
                    
                    let content = self.graph.node(idx).unwrap().payload().content.clone();
                    let label = self.graph.node(idx).unwrap().payload().label.clone();
                    
                    // Sync content_edit_buffer with node content ONLY when switching to a different node
                    // This prevents the buffer from being overwritten while editing
                    let node_changed = self.last_edited_node != Some(idx);
                    let mut just_synced = false;
                    if node_changed {
                        self.content_edit_buffer = content.clone();
                        self.last_edited_node = Some(idx);
                        just_synced = true;
                        // Close slash menu when switching nodes
                        self.slash_menu_open = false;
                        self.slash_menu_query.clear();
                    }

                    match self.sidebar_tab {
                        SidebarTab::Preview => {
                            // Preview Tab with clickable wikilinks
                            let available_height = ui.available_height() - 100.0;
                            
                            // First, extract wikilinks from content for click handling
                            let wikilinks: Vec<(String, Option<String>)> = self.wikilink_regex
                                .captures_iter(&self.content_edit_buffer)
                                .map(|cap| {
                                    let target = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                                    let label = cap.get(2).map(|m| m.as_str().to_string());
                                    (target, label)
                                })
                                .collect();
                            
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.set_min_height(available_height.max(200.0));
                                
                                // Show markdown preview
                                CommonMarkViewer::new()
                                    .show(ui, &mut self.markdown_cache, &self.content_edit_buffer);
                                
                                // Show clickable wikilinks section if any exist
                                if !wikilinks.is_empty() {
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.heading("🔗 Links in this node");
                                    
                                    for (target, label) in &wikilinks {
                                        let display = label.as_ref().unwrap_or(target);
                                        
                                        // Check if it's a URL
                                        let is_url = target.starts_with("http://") || 
                                                    target.starts_with("https://") ||
                                                    target.starts_with("www.");
                                        
                                        if is_url {
                                            // External link - blue and opens in browser
                                            if ui.add(egui::Label::new(
                                                egui::RichText::new(format!("🌐 {}", display))
                                                    .color(egui::Color32::from_rgb(100, 149, 237))
                                            ).sense(egui::Sense::click())).clicked() {
                                                let url = if target.starts_with("www.") {
                                                    format!("https://{}", target)
                                                } else {
                                                    target.clone()
                                                };
                                                #[cfg(not(target_arch = "wasm32"))]
                                                { let _ = webbrowser::open(&url); }
                                                #[cfg(target_arch = "wasm32")]
                                                {
                                                    if let Some(window) = web_sys::window() {
                                                        let _ = window.open_with_url(&url);
                                                    }
                                                }
                                            }
                                        } else {
                                            // Internal node link - blue and focuses node
                                            if ui.add(egui::Label::new(
                                                egui::RichText::new(format!("📄 {}", display))
                                                    .color(egui::Color32::from_rgb(100, 149, 237))
                                            ).sense(egui::Sense::click())).clicked() {
                                                // Find and select the target node
                                                let target_idx = self.graph.g().node_indices().find(|nidx| {
                                                    self.graph.node(*nidx).unwrap().payload().label == *target
                                                });
                                                if let Some(tidx) = target_idx {
                                                    self.graph.set_selected_nodes(vec![tidx]);
                                                    self.fit_to_view_next = true;
                                                    self.push_toast(format!("Jumped to: {}", target));
                                                } else {
                                                    self.push_toast(format!("Node not found: {}", target));
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        }
                        SidebarTab::Lua => {
                            // Lua Integration Tab - Comprehensive Guide
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.heading("🔧 Lua Scripting Guide");
                                ui.separator();
                                
                                ui.label("LogMark embeds a Lua interpreter for automating graph operations, \
                                         batch processing, and dynamic content generation.");
                                ui.add_space(8.0);
                                
                                // Quick Start Section
                                ui.collapsing("📖 Quick Start", |ui| {
                                    ui.label("Add a Lua code block to any node using the slash menu (/) or manually:");
                                    ui.code("```lua\n-- Your code here\nprint('Hello LogMark!')\n```");
                                    ui.add_space(4.0);
                                    ui.label("Then click ▶ Run below to execute the script.");
                                });
                                
                                ui.add_space(4.0);
                                
                                // Complete API Reference
                                ui.collapsing("📚 Graph API Reference", |ui| {
                                    ui.heading("Node Operations");
                                    egui::Grid::new("node_api").striped(true).show(ui, |ui| {
                                        ui.code("graph.add_node('label')"); ui.label("Create a new node"); ui.end_row();
                                        ui.code("graph.remove_node('label')"); ui.label("Delete a node"); ui.end_row();
                                        ui.code("graph.list_nodes()"); ui.label("Print all node labels"); ui.end_row();
                                        ui.code("graph.node_count()"); ui.label("Get total node count"); ui.end_row();
                                        ui.code("graph.get_node_content('label')"); ui.label("Get node content"); ui.end_row();
                                        ui.code("graph.set_node_content('label', 'content')"); ui.label("Set node content"); ui.end_row();
                                        ui.code("graph.get_neighbors('label')"); ui.label("List connected nodes"); ui.end_row();
                                    });
                                    
                                    ui.add_space(8.0);
                                    ui.heading("Edge Operations");
                                    egui::Grid::new("edge_api").striped(true).show(ui, |ui| {
                                        ui.code("graph.add_edge('from', 'to', 'label')"); ui.label("Connect two nodes"); ui.end_row();
                                        ui.code("graph.list_edges()"); ui.label("Print all edges"); ui.end_row();
                                        ui.code("graph.edge_count()"); ui.label("Get total edge count"); ui.end_row();
                                    });
                                    
                                    ui.add_space(8.0);
                                    ui.heading("Selection & Navigation");
                                    egui::Grid::new("select_api").striped(true).show(ui, |ui| {
                                        ui.code("graph.get_selected()"); ui.label("Get currently selected node"); ui.end_row();
                                        ui.code("graph.select('label')"); ui.label("Select a node by label"); ui.end_row();
                                        ui.code("graph.find_path('from', 'to')"); ui.label("Find shortest path"); ui.end_row();
                                    });
                                    
                                    ui.add_space(8.0);
                                    ui.heading("Variables & Output");
                                    egui::Grid::new("var_api").striped(true).show(ui, |ui| {
                                        ui.code("local x = 'value'"); ui.label("Create a variable"); ui.end_row();
                                        ui.code("print(x)"); ui.label("Print variable or string"); ui.end_row();
                                    });
                                });
                                
                                ui.add_space(4.0);
                                
                                // Use Case Examples
                                ui.collapsing("💡 Example Scripts", |ui| {
                                    ui.heading("1. Create a project structure");
                                    ui.code("```lua\n-- Create a software project graph\ngraph.add_node('Frontend')\ngraph.add_node('Backend')\ngraph.add_node('Database')\ngraph.add_edge('Frontend', 'Backend', 'API')\ngraph.add_edge('Backend', 'Database', 'SQL')\n```");
                                    
                                    ui.add_space(8.0);
                                    ui.heading("2. Batch create nodes from list");
                                    ui.code("```lua\n-- Create multiple related nodes\nlocal topics = 'Rust,Python,JavaScript'\n-- Then add nodes manually for each\ngraph.add_node('Rust')\ngraph.add_node('Python')\ngraph.add_node('JavaScript')\n```");
                                    
                                    ui.add_space(8.0);
                                    ui.heading("3. Graph analysis");
                                    ui.code("```lua\n-- Analyze graph structure\ngraph.node_count()\ngraph.edge_count()\ngraph.list_nodes()\ngraph.list_edges()\n```");
                                    
                                    ui.add_space(8.0);
                                    ui.heading("4. Find relationships");
                                    ui.code("```lua\n-- Find path between concepts\ngraph.find_path('Lua', 'LogMark')\ngraph.get_neighbors('LogMark')\n```");
                                    
                                    ui.add_space(8.0);
                                    ui.heading("5. Document generation");
                                    ui.code("```lua\n-- Set content programmatically\ngraph.set_node_content('README', '# My Project\\n\\nGenerated by Lua')\n```");
                                });
                                
                                ui.add_space(4.0);
                                
                                // Use Cases
                                ui.collapsing("🎯 Use Cases for Lua in LogMark", |ui| {
                                    ui.label("• Batch Operations: Create many nodes/edges at once");
                                    ui.label("• Templates: Generate standard graph structures");
                                    ui.label("• Analysis: Count, list, and explore graph data");
                                    ui.label("• Automation: Build graphs from external data");
                                    ui.label("• Documentation: Generate content programmatically");
                                    ui.label("• Navigation: Find paths and relationships");
                                    ui.label("• Refactoring: Bulk rename or restructure nodes");
                                    ui.label("• Export: Generate reports from graph data");
                                });
                                
                                ui.separator();
                                ui.heading("Lua Controls");
                                
                                if self.content_edit_buffer.contains("```lua") {
                                    ui.horizontal(|ui| {
                                        if ui.button("▶ Run").clicked() {
                                            if let Some(cap) = self.lua_regex.captures(&self.content_edit_buffer) {
                                                if let Some(script) = cap.get(1) {
                                                    let script_text = script.as_str().to_string();
                                                    self.run_lua_script(&script_text);
                                                }
                                            } else {
                                                self.lua_output = "No Lua code block found.".to_string();
                                            }
                                        }
                                        if ui.button("🐞 Debug").clicked() {
                                            self.lua_sidebar_open = true;
                                        }
                                    });
                                    ui.separator();
                                    ui.label("Output:");
                                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                                        ui.code(&self.lua_output);
                                    });
                                    
                                    if !self.lua_variables.is_empty() {
                                        ui.separator();
                                        ui.heading("Variables");
                                        for (name, val) in &self.lua_variables {
                                            ui.horizontal(|ui| {
                                                ui.code(name);
                                                ui.label("=");
                                                ui.label(val);
                                            });
                                        }
                                    }
                                } else {
                                    ui.label("📝 No Lua block in current node.");
                                    ui.add_space(4.0);
                                    if ui.button("➕ Add Lua Block").clicked() {
                                        self.content_edit_buffer.push_str("\n\n```lua\n-- Your Lua script here\nprint('Hello!')\n```\n");
                                        if let Some(node) = self.graph.node_mut(idx) {
                                            node.payload_mut().content = self.content_edit_buffer.clone();
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
                            
                            // Main text editor
                            let available_height = ui.available_height() - 60.0;
                            // let editor_id = egui::Id::new("content_editor").with(idx);
                            
                            egui::ScrollArea::vertical().max_height(available_height.max(200.0)).show(ui, |ui| {
                                let response = CodeEditor::default()
                                    .id_source(format!("content_editor_{}", idx.index()))
                                    .with_syntax(syntax::markdown())
                                    .with_fontsize(14.0)
                                    .with_theme(ColorTheme::GRUVBOX)
                                    .show_with_completer(ui, &mut self.content_edit_buffer, &mut self.completer)
                                    .response;
                                
                                // Detect slash key press and open menu
                                // Only process changes if this is a real user edit, not just syncing to a new node
                                if response.changed() && !just_synced {
                                    // Check if user just typed '/'
                                    if let Some(state) = TextEdit::load_state(ui.ctx(), response.id) {
                                        if let Some(range) = state.cursor.char_range() {
                                            let cursor_idx = range.primary.index;
                                            self.cursor_position = cursor_idx;
                                            
                                            // Check if the character before cursor is '/'
                                            if cursor_idx > 0 {
                                                let chars: Vec<char> = self.content_edit_buffer.chars().collect();
                                                if cursor_idx <= chars.len() && chars.get(cursor_idx - 1) == Some(&'/') {
                                                    // Check it's at start of line or after whitespace
                                                    let prev_char = if cursor_idx >= 2 { chars.get(cursor_idx - 2) } else { None };
                                                    let is_valid_trigger = prev_char.is_none() || prev_char == Some(&'\n') || prev_char.map(|c| c.is_whitespace()) == Some(true);
                                                    
                                                    // Also check we are NOT in a code block
                                                    let is_in_code = self.parser.is_in_code_block(&self.content_edit_buffer, cursor_idx);
                                                    
                                                    if is_valid_trigger && !is_in_code {
                                                        self.slash_menu_open = true;
                                                        self.slash_menu_selection = 0;
                                                        self.slash_menu_query.clear();
                                                        if let Some(pos) = response.interact_pointer_pos() {
                                                            self.slash_menu_pos = Some(pos);
                                                        } else {
                                                            self.slash_menu_pos = Some(response.rect.left_top() + egui::Vec2::new(20.0, 20.0));
                                                        }
                                                    }
                                                } else if self.slash_menu_open {
                                                    // If menu is open and user types space, close it (keep the /)
                                                    if chars.get(cursor_idx.saturating_sub(1)) == Some(&' ') {
                                                        self.slash_menu_open = false;
                                                        self.slash_menu_query.clear();
                                                    } else {
                                                        // Update query with characters after /
                                                        // Find the / and get everything between / and cursor
                                                        let mut slash_pos = None;
                                                        for i in (0..cursor_idx).rev() {
                                                            if chars.get(i) == Some(&'/') {
                                                                slash_pos = Some(i);
                                                                break;
                                                            } else if chars.get(i).map(|c| c.is_whitespace()) == Some(true) && chars.get(i) != Some(&' ') {
                                                                break;
                                                            }
                                                        }
                                                        if let Some(sp) = slash_pos {
                                                            self.slash_menu_query = chars.iter().skip(sp + 1).take(cursor_idx - sp - 1).collect();
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // Auto-completion for [[ - REMOVED to prevent premature node creation
                                            /*
                                            if cursor_idx >= 2 {
                                                let chars: Vec<char> = self.content_edit_buffer.chars().collect();
                                                if chars.get(cursor_idx.saturating_sub(2)..cursor_idx) == Some(&['[', '[']) {
                                                    let next_char = chars.get(cursor_idx);
                                                    if next_char != Some(&']') {
                                                        let mut new_content = String::new();
                                                        new_content.extend(chars.iter().take(cursor_idx));
                                                        new_content.push_str("]]");
                                                        new_content.extend(chars.iter().skip(cursor_idx));
                                                        self.content_edit_buffer = new_content;
                                                    }
                                                }
                                            }
                                            */
                                        }
                                    }
                                    
                                    // Save changes to node and handle wikilinks
                                    if self.change_count % 5 == 0 {
                                        self.push_undo();
                                    }
                                    self.change_count += 1;
                                    if let Some(node) = self.graph.node_mut(idx) {
                                        node.payload_mut().content = self.content_edit_buffer.clone();
                                    }
                                    self.handle_wikilinks(idx);
                                }
                            });
                            
                            // Slash Menu Popup
                            if self.slash_menu_open {
                                let menu_pos = self.slash_menu_pos.unwrap_or(egui::Pos2::new(100.0, 100.0));
                                
                                // Filter items based on query - collect owned data to avoid borrow issues
                                let query_lower = self.slash_menu_query.to_lowercase();
                                let filtered_items: Vec<(usize, String, String, String)> = self.slash_menu_items.iter()
                                    .enumerate()
                                    .filter(|(_, (_, item_label, _))| {
                                        query_lower.is_empty() || 
                                        item_label.to_lowercase().contains(&query_lower)
                                    })
                                    .map(|(orig_idx, (icon, item_label, template))| {
                                        (orig_idx, icon.to_string(), item_label.to_string(), template.to_string())
                                    })
                                    .collect();
                                
                                let num_items = filtered_items.len();
                                let mut clicked_item: Option<usize> = None;
                                
                                egui::Area::new(egui::Id::new("slash_menu"))
                                    .fixed_pos(menu_pos)
                                    .order(egui::Order::Foreground)
                                    .show(ui.ctx(), |ui| {
                                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                                            ui.set_min_width(200.0);
                                            ui.set_max_height(300.0);
                                            
                                            ui.horizontal(|ui| {
                                                ui.label("⚡");
                                                let query_response = ui.add(
                                                    TextEdit::singleline(&mut self.slash_menu_query)
                                                        .hint_text("Search...")
                                                );
                                                // If Enter was pressed in the query field, trigger insert
                                                if query_response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                                    clicked_item = Some(self.slash_menu_selection);
                                                }
                                            });
                                            
                                            ui.separator();
                                            
                                            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                                                for (display_idx, (orig_idx, icon, item_label, _template)) in filtered_items.iter().enumerate() {
                                                    let is_selected = display_idx == self.slash_menu_selection;
                                                    let response = ui.selectable_label(is_selected, format!("{} {}", icon, item_label));
                                                    
                                                    if response.clicked() {
                                                        clicked_item = Some(*orig_idx);
                                                    }
                                                }
                                            });
                                        });
                                    });
                                
                                // Handle clicked item after UI is drawn
                                if let Some(orig_idx) = clicked_item {
                                    self.slash_menu_selection = orig_idx;
                                    self.insert_slash_template(idx);
                                }
                                
                                // Handle keyboard navigation
                                let max_selection = num_items.saturating_sub(1);
                                if ui.ctx().input(|i| i.key_pressed(Key::ArrowDown)) {
                                    self.slash_menu_selection = (self.slash_menu_selection + 1).min(max_selection);
                                }
                                if ui.ctx().input(|i| i.key_pressed(Key::ArrowUp)) {
                                    self.slash_menu_selection = self.slash_menu_selection.saturating_sub(1);
                                }
                                if ui.ctx().input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Tab)) {
                                    self.insert_slash_template(idx);
                                }
                                if ui.ctx().input(|i| i.key_pressed(Key::Escape)) {
                                    self.slash_menu_open = false;
                                }
                                
                                // Close menu if clicked elsewhere
                                if ui.ctx().input(|i| i.pointer.any_click()) && !ui.ctx().is_pointer_over_area() {
                                    self.slash_menu_open = false;
                                }
                            }
                        }
                    }
                } else {
                    ui.label("Select a node to edit its content.");
                }
            });
        }
        
        // Auto-rotate for 3D view
        if self.visualization_mode == VisualizationMode::ThreeD && self.auto_rotate {
            self.rotation_y += self.rotation_speed;
            if self.rotation_y > 180.0 {
                self.rotation_y -= 360.0;
            }
            ctx.request_repaint();
        }
            
        // Central Panel - based on current tab
        CentralPanel::default().show(ctx, |ui| {
            match self.current_tab {
                AppTab::Graph => {
                    // Graph View
                    let interactions = SettingsInteraction::default()
                        .with_dragging_enabled(true)
                        .with_node_selection_enabled(true)
                        .with_node_selection_multi_enabled(false)
                        .with_edge_selection_enabled(true)
                        .with_edge_selection_multi_enabled(false)
                        .with_edge_clicking_enabled(true)
                        .with_node_clicking_enabled(false); // Disable node clicking to spawn nodes

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
                    
                    // Draw 3D overlay if in 3D mode
                    if self.visualization_mode == VisualizationMode::ThreeD {
                        let painter = ui.painter();
                        let rect = ui.available_rect_before_wrap();
                        let _pivot = rect.center();
                        
                        // Draw a visual indicator for 3D mode
                        painter.text(
                            egui::Pos2::new(rect.left() + 10.0, rect.top() + 10.0),
                            egui::Align2::LEFT_TOP,
                            "🌐 3D View",
                            egui::FontId::default(),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180),
                        );
                    }
                    
                    // Draw cardinality labels below edges if enabled
                    // Note: This is drawn as an overlay since DisplayEdge doesn't have access to graph structure
                    if self.show_cardinality {
                        // Get the metadata to transform graph coordinates to screen coordinates
                        let layout_id = match self.layout {
                            AppLayout::Random => Some("random".to_string()),
                            AppLayout::Force => Some("force".to_string()), 
                            AppLayout::Hierarchical => Some("hierarchical".to_string()),
                        };
                        
                        // Load the metadata frame to get zoom and pan
                        let meta = MetadataFrame::new(layout_id).load(ui);
                        
                        // Get the widget rect to calculate screen offset
                        let widget_rect = resp.rect;
                        
                        // We need to get edge positions and draw cardinality
                        let painter = ui.painter();
                        
                        for edge_ref in self.graph.g().edge_references() {
                            let source_idx = edge_ref.source();
                            let target_idx = edge_ref.target();
                            
                            if let (Some(source_node), Some(target_node)) = (self.graph.node(source_idx), self.graph.node(target_idx)) {
                                let start_graph = source_node.location();
                                let end_graph = target_node.location();
                                
                                // Transform graph coordinates to screen coordinates
                                // Use MetadataFrame's canvas_to_screen_pos and add widget offset
                                let start_canvas = meta.canvas_to_screen_pos(start_graph);
                                let end_canvas = meta.canvas_to_screen_pos(end_graph);
                                
                                let start_pos = egui::Pos2::new(
                                    start_canvas.x + widget_rect.min.x,
                                    start_canvas.y + widget_rect.min.y,
                                );
                                let end_pos = egui::Pos2::new(
                                    end_canvas.x + widget_rect.min.x,
                                    end_canvas.y + widget_rect.min.y,
                                );
                                
                                // Calculate midpoint in screen space
                                let mid = start_pos + (end_pos - start_pos) * 0.5;
                                
                                // Get cardinality
                                let cardinality = self.calculate_edge_cardinality(source_idx, target_idx);
                                
                                // Calculate perpendicular direction for offset
                                let dir = (end_pos - start_pos).normalized();
                                let perp = egui::Vec2::new(dir.y, -dir.x); // Perpendicular to edge direction
                                
                                // Offset further from edge to avoid label overlap (edge labels are at midpoint)
                                let cardinality_pos = mid + perp * 35.0 + dir * 15.0;
                                
                                // Only draw if within visible bounds
                                if widget_rect.contains(cardinality_pos) {
                                    // Color based on cardinality type
                                    let color = match cardinality.as_str() {
                                        "1:1" => egui::Color32::from_rgb(100, 200, 100), // Green
                                        "1:N" | "N:1" => egui::Color32::from_rgb(200, 200, 100), // Yellow
                                        "N:N" => egui::Color32::from_rgb(200, 100, 100), // Red
                                        _ => egui::Color32::LIGHT_GRAY,
                                    };
                                    
                                    // Draw background
                                    let text_galley = painter.layout_no_wrap(
                                        cardinality.clone(),
                                        egui::FontId::new(10.0, egui::FontFamily::Monospace),
                                        color,
                                    );
                                    let bg_rect = egui::Rect::from_center_size(
                                        cardinality_pos, 
                                        text_galley.size() + egui::Vec2::new(6.0, 4.0)
                                    );
                                    painter.rect_filled(bg_rect, 3.0, egui::Color32::from_black_alpha(200));
                                    
                                    // Draw cardinality text
                                    painter.text(
                                        cardinality_pos,
                                        egui::Align2::CENTER_CENTER,
                                        cardinality,
                                        egui::FontId::new(10.0, egui::FontFamily::Monospace),
                                        color,
                                    );
                                }
                            }
                        }
                    }
                }
                
                AppTab::Preview => {
                    // Full document preview
                    ui.heading("Document Preview");
                    ui.separator();
                    
                    // Collect all node data first to avoid borrow conflicts
                    let nodes_data: Vec<_> = self.graph.g().node_indices()
                        .filter_map(|node_idx| {
                            self.graph.node(node_idx).map(|node| {
                                let payload = node.payload();
                                let edges: Vec<_> = self.graph.g().edges(node_idx)
                                    .filter_map(|edge| {
                                        self.graph.node(edge.target()).map(|target_node| {
                                            let edge_label = edge.weight().label();
                                            let label_text = if edge_label.is_empty() {
                                                format!("→ {}", target_node.payload().label)
                                            } else {
                                                format!("{} → {}", edge_label, target_node.payload().label)
                                            };
                                            (edge.target(), label_text)
                                        })
                                    })
                                    .collect();
                                (node_idx, payload.label.clone(), payload.content.clone(), edges)
                            })
                        })
                        .collect();
                    
                    // Store clicked link target for later processing
                    let mut clicked_target: Option<petgraph::stable_graph::NodeIndex> = None;
                    
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (node_idx, label, content, edges) in nodes_data.iter() {
                            let _node_idx = *node_idx;
                            
                            ui.group(|ui| {
                                ui.heading(label);
                                CommonMarkViewer::new()
                                    .show(ui, &mut self.markdown_cache, content);
                                
                                if !edges.is_empty() {
                                    ui.separator();
                                    ui.horizontal(|ui| {
                                        ui.label("Links:");
                                        for (target_idx, label_text) in edges {
                                            if ui.link(label_text).clicked() {
                                                clicked_target = Some(*target_idx);
                                            }
                                        }
                                    });
                                }
                            });
                            ui.add_space(8.0);
                        }
                    });
                    
                    // Handle the clicked link after the scroll area
                    if let Some(target_idx) = clicked_target {
                        self.graph.set_selected_nodes(vec![target_idx]);
                        self.current_tab = AppTab::Graph;
                        self.fit_to_view_next = true;
                    }
                }
            }
        });
    }
}
