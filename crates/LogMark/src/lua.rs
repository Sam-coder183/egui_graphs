use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};
use egui_graphs::Graph;
use petgraph::Directed;
use egui::Pos2;

pub struct LuaEngine {
    pub output: String,
    pub variables: Vec<(String, String)>,
}

impl LuaEngine {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            variables: Vec::new(),
        }
    }

    pub fn run_script(
        &mut self,
        script: &str,
        graph: &mut Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>,
        change_count: &mut usize,
        undo_stack: &mut Vec<Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>>,
        redo_stack: &mut Vec<Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>>,
    ) {
        self.output.clear();
        self.variables.clear();
        self.output.push_str("Running script...\n");

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
                    self.variables.iter()
                        .find(|(k, _)| k == content)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_else(|| "nil".to_string())
                };
                self.output.push_str(&format!("> {}\n", val));
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
                    if let Some(existing) = self.variables.iter_mut().find(|(k, _)| k == &var_name) {
                        existing.1 = val;
                    } else {
                        self.variables.push((var_name, val));
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
                
                // Push undo
                if undo_stack.len() > 50 {
                    undo_stack.remove(0);
                }
                undo_stack.push(graph.clone());
                redo_stack.clear();

                let idx = graph.add_node(LogNodeData { 
                    label: label.clone(), 
                    content: format!("# {}\nCreated by Lua script.", label) 
                });
                graph.node_mut(idx).unwrap().set_location(Pos2::new(0.0, 0.0));
                self.output.push_str(&format!("Added node: {}\n", label));
                *change_count += 1;
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

                    let from_idx = graph.g().node_indices().find(|idx| {
                        graph.node(*idx).unwrap().payload().label == from_label
                    });
                    let to_idx = graph.g().node_indices().find(|idx| {
                        graph.node(*idx).unwrap().payload().label == to_label
                    });

                    if let (Some(from), Some(to)) = (from_idx, to_idx) {
                        // Push undo
                        if undo_stack.len() > 50 {
                            undo_stack.remove(0);
                        }
                        undo_stack.push(graph.clone());
                        redo_stack.clear();

                        graph.add_edge(from, to, LogEdgeData { label: edge_label });
                        self.output.push_str(&format!("Added edge: {} -> {}\n", from_label, to_label));
                        *change_count += 1;
                    } else {
                        self.output.push_str(&format!("Error: Nodes not found: {} -> {}\n", from_label, to_label));
                    }
                }
            }
            // graph.list_nodes()
            else if line == "graph.list_nodes()" {
                self.output.push_str("Nodes:\n");
                for idx in graph.g().node_indices() {
                    let label = &graph.node(idx).unwrap().payload().label;
                    self.output.push_str(&format!("- {}\n", label));
                }
            }
            // graph.list_edges()
            else if line == "graph.list_edges()" {
                self.output.push_str("Edges:\n");
                for edge_idx in graph.g().edge_indices() {
                    if let Some(edge) = graph.edge(edge_idx) {
                        let (source, target) = graph.edge_endpoints(edge_idx).unwrap();
                        let source_label = &graph.node(source).unwrap().payload().label;
                        let target_label = &graph.node(target).unwrap().payload().label;
                        let label = edge.payload().label.as_deref().unwrap_or("");
                        self.output.push_str(&format!("- {} -> {} ({})\n", source_label, target_label, label));
                    }
                }
            }
            // graph.node_count()
            else if line == "graph.node_count()" {
                let count = graph.g().node_count();
                self.output.push_str(&format!("> {}\n", count));
            }
            // graph.edge_count()
            else if line == "graph.edge_count()" {
                let count = graph.g().edge_count();
                self.output.push_str(&format!("> {}\n", count));
            }
            // graph.get_node_content("label")
            else if line.starts_with("graph.get_node_content(") && line.ends_with(')') {
                let content = &line[23..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = graph.g().node_indices().find(|idx| {
                    graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    let content = &graph.node(idx).unwrap().payload().content;
                    self.output.push_str(&format!("> {}\n", content));
                } else {
                    self.output.push_str(&format!("Error: Node not found: {}\n", label));
                }
            }
            // graph.set_node_content("label", "content")
            else if line.starts_with("graph.set_node_content(") && line.ends_with(')') {
                let args = &line[23..line.len()-1];
                let parts: Vec<&str> = args.splitn(2, ',').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    let label = parts[0].trim_matches(|c| c == '\'' || c == '"');
                    let new_content = parts[1].trim_matches(|c| c == '\'' || c == '"');
                    let node = graph.g().node_indices().find(|idx| {
                        graph.node(*idx).unwrap().payload().label == label
                    });
                    if let Some(idx) = node {
                        // Push undo
                        if undo_stack.len() > 50 {
                            undo_stack.remove(0);
                        }
                        undo_stack.push(graph.clone());
                        redo_stack.clear();

                        graph.node_mut(idx).unwrap().payload_mut().content = new_content.to_string();
                        self.output.push_str(&format!("Updated content for: {}\n", label));
                        *change_count += 1;
                    } else {
                        self.output.push_str(&format!("Error: Node not found: {}\n", label));
                    }
                }
            }
            // graph.remove_node("label")
            else if line.starts_with("graph.remove_node(") && line.ends_with(')') {
                let content = &line[18..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = graph.g().node_indices().find(|idx| {
                    graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    // Push undo
                    if undo_stack.len() > 50 {
                        undo_stack.remove(0);
                    }
                    undo_stack.push(graph.clone());
                    redo_stack.clear();

                    graph.remove_node(idx);
                    self.output.push_str(&format!("Removed node: {}\n", label));
                    *change_count += 1;
                } else {
                    self.output.push_str(&format!("Error: Node not found: {}\n", label));
                }
            }
            // graph.get_neighbors("label")
            else if line.starts_with("graph.get_neighbors(") && line.ends_with(')') {
                let content = &line[20..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = graph.g().node_indices().find(|idx| {
                    graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    self.output.push_str(&format!("Neighbors of {}:\n", label));
                    for neighbor in graph.g().neighbors(idx) {
                        let n_label = &graph.node(neighbor).unwrap().payload().label;
                        self.output.push_str(&format!("- {}\n", n_label));
                    }
                } else {
                    self.output.push_str(&format!("Error: Node not found: {}\n", label));
                }
            }
            // graph.find_path("from", "to")
            else if line.starts_with("graph.find_path(") && line.ends_with(')') {
                let args = &line[16..line.len()-1];
                let parts: Vec<&str> = args.split(',').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    let from_label = parts[0].trim_matches(|c| c == '\'' || c == '"');
                    let to_label = parts[1].trim_matches(|c| c == '\'' || c == '"');
                    let from_idx = graph.g().node_indices().find(|idx| {
                        graph.node(*idx).unwrap().payload().label == from_label
                    });
                    let to_idx = graph.g().node_indices().find(|idx| {
                        graph.node(*idx).unwrap().payload().label == to_label
                    });
                    if let (Some(from), Some(to)) = (from_idx, to_idx) {
                        let path = petgraph::algo::astar(
                            graph.g(), 
                            from, 
                            |finish| finish == to, 
                            |_| 1, 
                            |_| 0
                        );
                        if let Some((cost, path)) = path {
                            self.output.push_str(&format!("Path found (cost {}):\n", cost));
                            for node_idx in path {
                                let label = &graph.node(node_idx).unwrap().payload().label;
                                self.output.push_str(&format!("-> {}\n", label));
                            }
                        } else {
                            self.output.push_str("No path found.\n");
                        }
                    } else {
                        self.output.push_str(&format!("Error: Nodes not found: {} -> {}\n", from_label, to_label));
                    }
                }
            }
            // graph.get_selected()
            else if line == "graph.get_selected()" {
                if let Some(idx) = graph.selected_nodes().first() {
                    let label = &graph.node(*idx).unwrap().payload().label;
                    self.output.push_str(&format!("> {}\n", label));
                } else {
                    self.output.push_str("> nil\n");
                }
            }
            // graph.select("label")
            else if line.starts_with("graph.select(") && line.ends_with(')') {
                let content = &line[13..line.len()-1];
                let label = content.trim_matches(|c| c == '\'' || c == '"');
                let node = graph.g().node_indices().find(|idx| {
                    graph.node(*idx).unwrap().payload().label == label
                });
                if let Some(idx) = node {
                    graph.node_mut(idx).unwrap().set_selected(true);
                    self.output.push_str(&format!("Selected: {}\n", label));
                } else {
                    self.output.push_str(&format!("Error: Node not found: {}\n", label));
                }
            }
        }
        self.output.push_str("Done.");
    }
}
