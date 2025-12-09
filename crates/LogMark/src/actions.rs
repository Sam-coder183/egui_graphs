use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};
use egui_graphs::Graph;
use petgraph::Directed;
use petgraph::visit::EdgeRef;
use crate::parser::MarkdownParser;
use regex::Regex;
use std::collections::HashMap;
use egui::Vec2;

pub fn handle_wikilinks(
    graph: &mut Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>,
    node_idx: petgraph::stable_graph::NodeIndex,
    parser: &mut MarkdownParser,
    wikilink_regex: &Regex,
    create_missing_nodes: bool
) {
    let content = graph.node(node_idx).unwrap().payload().content.clone();
    
    // 1. Parse current wikilinks from content using Tree-sitter to exclude code blocks
    let mut current_links = HashMap::new();
    
    // Get safe ranges (not code blocks)
    let safe_ranges = parser.get_safe_ranges(&content);
    
    for range in safe_ranges {
        let slice = &content[range];
        for cap in wikilink_regex.captures_iter(slice) {
            if let Some(m) = cap.get(1) {
                let target_label = m.as_str().to_string();
                let edge_label = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| "links to".to_string());
                current_links.insert(target_label, edge_label);
            }
        }
    }

    // 2. Identify existing edges from this node
    let mut edges_to_remove = Vec::new();
    let mut existing_targets = HashMap::new(); // target_idx -> edge_idx

    for edge in graph.g().edges(node_idx) {
        let target_idx = edge.target();
        if target_idx == node_idx { continue; }
        existing_targets.insert(target_idx, edge.id());
    }

    // 3. Process edges to remove (those not in current_links)
    for (target_idx, edge_idx) in &existing_targets {
        if let Some(target_node) = graph.node(*target_idx) {
            let target_label = &target_node.payload().label;
            if !current_links.contains_key(target_label) {
                edges_to_remove.push((*edge_idx, *target_idx));
            }
        }
    }

    // Remove edges and cleanup orphans
    for (edge_idx, target_idx) in edges_to_remove {
        graph.g_mut().remove_edge(edge_idx);
        
        // Check if target node is now an orphan and should be deleted
        let is_orphan = graph.g().edges_directed(target_idx, petgraph::Direction::Incoming).count() == 0;
        if is_orphan {
            if let Some(node) = graph.node(target_idx) {
                let default_content = format!("# {}", node.payload().label);
                if node.payload().content == default_content {
                    graph.remove_node(target_idx);
                }
            }
        }
    }

    // 4. Process current links (add or update edges)
    for (target_label, edge_label) in current_links {
        let mut target_idx = None;
        for idx in graph.g().node_indices() {
            if let Some(node) = graph.node(idx) {
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
                if !create_missing_nodes {
                    continue;
                }
                let new_node_data = LogNodeData {
                    label: target_label.clone(),
                    content: format!("# {}", target_label),
                };
                let idx = graph.add_node(new_node_data);
                let source_pos = graph.node(node_idx).unwrap().location();
                graph.node_mut(idx).unwrap().set_location(source_pos + Vec2::new(50.0, 50.0));
                idx
            }
        };

        if target_idx == node_idx { continue; }

        if let Some(&edge_idx) = existing_targets.get(&target_idx) {
            if let Some(edge) = graph.edge_mut(edge_idx) {
                if edge.payload().label.as_ref() != Some(&edge_label) {
                    edge.payload_mut().label = Some(edge_label);
                }
            }
        } else {
            graph.add_edge(node_idx, target_idx, LogEdgeData { label: Some(edge_label), cardinality: None });
        }
    }
}

pub fn cleanup_orphans(
    graph: &mut Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>,
) -> usize {
    let mut removed_count = 0;
    let mut nodes_to_remove = Vec::new();

    for idx in graph.g().node_indices() {
        // Check if node has any incoming or outgoing edges
        let has_edges = graph.g().edges(idx).count() > 0 || 
                       graph.g().edges_directed(idx, petgraph::Direction::Incoming).count() > 0;
        
        if !has_edges {
            // Only remove if it looks like an auto-generated node (simple content)
            // OR if the user explicitly requested cleanup, maybe we should be more aggressive?
            // For now, let's stick to the safety check: only remove if content is just the header
            if let Some(node) = graph.node(idx) {
                let default_content = format!("# {}\n", node.payload().label);
                let default_content_trim = format!("# {}", node.payload().label);
                
                // Check against both formats (with and without newline)
                if node.payload().content == default_content || 
                   node.payload().content == default_content_trim ||
                   node.payload().content.trim() == default_content_trim {
                    nodes_to_remove.push(idx);
                }
            }
        }
    }

    for idx in nodes_to_remove {
        graph.remove_node(idx);
        removed_count += 1;
    }

    removed_count
}
