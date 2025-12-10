use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};
use egui_graphs::Graph;
use petgraph::{stable_graph::StableGraph, Directed};
use ron::de::from_str;
use ron::ser::{to_string_pretty, PrettyConfig};
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, path::PathBuf};
#[cfg(target_arch = "wasm32")]
use web_sys::Storage;

pub type LogGraph = Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>;

#[derive(Serialize, Deserialize)]
pub struct ProjectState {
    pub doc_graph: LogGraph,
    pub code_graph: LogGraph,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn autosave_path() -> PathBuf {
    PathBuf::from("logmark_autosave.ron")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn default_graph_path() -> PathBuf {
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

pub fn default_graph() -> LogGraph {
    // First, try to load from external file
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = default_graph_path();
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
    hardcoded_default_graph()
}

pub fn hardcoded_default_graph() -> LogGraph {
    let mut g = StableGraph::new();

    let overview = g.add_node(LogNodeData {
        label: "LogMark".to_string(),
        content: "# LogMark\n\nWelcome to LogMark, a graph-based note app.\n\nBrowse nodes to learn features. Select a node to edit on the right.\n\n[[Auto-Commit|explains]]\n[[Layouts|shows]]\n[[Relationships|details]]\n[[Slash Menu|includes]]\n[[Lua|supports]]\n[[Hierarchy|adds]]".to_string(),
    });

    let auto_commit = g.add_node(LogNodeData {
        label: "Auto-Commit".to_string(),
        content: "# Auto-Commit\n\nEdits increment a counter; after 10 the app auto-commits.\nYou can keep typing—commits are automatic.".to_string(),
    });

    let layouts = g.add_node(LogNodeData {
        label: "Layouts".to_string(),
        content: "# Layouts\n\nSwitch layouts from the left Options bar.\n- Random: scattered.\n- Force: physics.\n- Hierarchical: tree view to fill the screen.\n\n[[Hierarchy|offers]]".to_string(),
    });

    let relationships = g.add_node(LogNodeData {
        label: "Relationships".to_string(),
        content: "# Relationships\n\nEdges show labels above arrows.\nUse `[[WikiLinks]]` or the slash menu to connect notes.".to_string(),
    });

    let slash = g.add_node(LogNodeData {
        label: "Slash Menu".to_string(),
        content: "# Slash Menu\n\nType `/` to open. Navigate with ↑/↓. Press Enter or Tab to accept. Options: headings, Lua block, or link to another node.\n\n[[Relationships|creates]]".to_string(),
    });

    let lua = g.add_node(LogNodeData {
        label: "Lua".to_string(),
        content: "# Lua Integration\n\nLogMark supports Lua scripting for dynamic content.\n\n## Usage\nCreate a code block with `lua` language:\n\n```lua\n-- Example\nlocal x = 10\nprint('Value:', x)\ngraph.add_node('New Node')\n```\n\n## Features\n- **Run**: Execute the script.\n- **Debug**: Inspect variables in the left sidebar.\n- **API**:\n  - `print(val)`: Print to output.\n  - `graph.add_node('label')`: Create a node.\n  - `graph.add_edge('from', 'to', 'label')`: Connect nodes.\n  - `graph.list_nodes()`: List all nodes.\n\n[[Relationships|annotates]]".to_string(),
    });

    let hierarchy = g.add_node(LogNodeData {
        label: "Hierarchy".to_string(),
        content: "# Hierarchy\n\nHierarchical layout provides a node tree view that can fill the screen.".to_string(),
    });

    g.add_edge(overview, auto_commit, LogEdgeData { label: Some("explains".to_string()), cardinality: None });
    g.add_edge(overview, layouts, LogEdgeData { label: Some("shows".to_string()), cardinality: None });
    g.add_edge(overview, relationships, LogEdgeData { label: Some("details".to_string()), cardinality: None });
    g.add_edge(overview, slash, LogEdgeData { label: Some("includes".to_string()), cardinality: None });
    g.add_edge(overview, lua, LogEdgeData { label: Some("supports".to_string()), cardinality: None });
    g.add_edge(overview, hierarchy, LogEdgeData { label: Some("adds".to_string()), cardinality: None });
    g.add_edge(slash, relationships, LogEdgeData { label: Some("creates".to_string()), cardinality: None });
    g.add_edge(layouts, hierarchy, LogEdgeData { label: Some("offers".to_string()), cardinality: None });
    g.add_edge(lua, relationships, LogEdgeData { label: Some("annotates".to_string()), cardinality: None });

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

pub fn load_autosave() -> Option<ProjectState> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = autosave_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    // Try loading as ProjectState first
                    if let Ok(state) = from_str::<ProjectState>(&content) {
                        println!("Loaded project state from {}", path.display());
                        return Some(state);
                    }
                    // Fallback: try loading as single Graph (legacy)
                    match from_str::<LogGraph>(&content) {
                        Ok(graph) => {
                            println!("Loaded legacy graph from {}", path.display());
                            // Convert to ProjectState with empty code graph
                            return Some(ProjectState {
                                doc_graph: graph,
                                code_graph: Graph::from(&StableGraph::new()),
                            });
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
        if let Some(storage) = web_storage() {
            if let Ok(Some(content)) = storage.get_item("logmark_autosave") {
                if let Ok(state) = from_str::<ProjectState>(&content) {
                    return Some(state);
                }
                if let Ok(graph) = from_str::<LogGraph>(&content) {
                    return Some(ProjectState {
                        doc_graph: graph,
                        code_graph: Graph::from(&StableGraph::new()),
                    });
                }
            }
        }
    }
    None
}

pub fn commit_changes(state: &ProjectState) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = autosave_path();
        let config = PrettyConfig::default();
        match to_string_pretty(state, config) {
            Ok(data) => {
                if let Err(err) = fs::write(&path, data) {
                    eprintln!("Auto-commit write failed: {}", err);
                }
            }
            Err(err) => eprintln!("Auto-commit serialization failed: {}", err),
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(storage) = web_storage() {
            let config = PrettyConfig::default();
            if let Ok(data) = to_string_pretty(state, config) {
                let _ = storage.set_item("logmark_autosave", &data);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn web_storage() -> Option<Storage> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
}
