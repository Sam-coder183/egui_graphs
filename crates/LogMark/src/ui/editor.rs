use egui::{Ui, TextEdit, ScrollArea, Id, Color32, Frame, Align};
use egui_code_editor::{CodeEditor, ColorTheme, Completer};
use crate::parser::MarkdownParser;
use crate::syntax;
use crate::graph::{LogNode, LogEdge, LogNodeData, LogEdgeData};
use egui_graphs::Graph;
use petgraph::Directed;
use regex::Regex;
use crate::actions::handle_wikilinks;

pub struct EditorState {
    pub content_buffer: String,
    pub cursor_position: usize,
    pub slash_menu_open: bool,
    pub slash_menu_query: String,
    pub slash_menu_pos: Option<egui::Pos2>,
    pub slash_menu_selection: usize,
    pub slash_menu_items: Vec<(&'static str, &'static str, &'static str)>,
    pub parser: MarkdownParser,
    pub completer: Completer,
    pub last_edited_node: Option<petgraph::stable_graph::NodeIndex>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            content_buffer: String::new(),
            cursor_position: 0,
            slash_menu_open: false,
            slash_menu_query: String::new(),
            slash_menu_pos: None,
            slash_menu_selection: 0,
            slash_menu_items: vec![
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
                ("📝", "Lua Block", "```lua\n|\n```"),
                ("🔧", "Lua: Add Node", "```lua\ngraph.add_node('|')\n```"),
                ("🔗", "Lua: Add Edge", "```lua\ngraph.add_edge('from', 'to', '|')\n```"),
                ("📊", "Lua: List Nodes", "```lua\ngraph.list_nodes()\n|\n```"),
                ("🔍", "Lua: Find Path", "```lua\ngraph.find_path('|', '')\n```"),
                ("📋", "Lua: Get Neighbors", "```lua\ngraph.get_neighbors('|')\n```"),
                ("✏️", "Lua: Set Content", "```lua\ngraph.set_node_content('|', 'content')\n```"),
            ],
            parser: MarkdownParser::new(),
            completer: Completer::new_with_syntax(&syntax::markdown()).with_user_words(),
            last_edited_node: None,
        }
    }

    pub fn show(
        &mut self, 
        ui: &mut Ui, 
        node_idx: petgraph::stable_graph::NodeIndex, 
        graph: &mut Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>, 
        change_count: &mut usize,
        wikilink_regex: &Regex,
        just_synced: bool
    ) {
        let mut current_cursor_idx = 0;
        
        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
            let response = CodeEditor::default()
                .id_source(format!("content_editor_{}", node_idx.index()))
                .with_syntax(syntax::markdown())
                .with_fontsize(14.0)
                .with_theme(ColorTheme::GRUVBOX)
                .show_with_completer(ui, &mut self.content_buffer, &mut self.completer)
                .response;
            
            // Detect slash key press and open menu
            // Only process changes if this is a real user edit, not just syncing to a new node
            if response.changed() && !just_synced {
                // Check if user just typed '/'
                if let Some(state) = TextEdit::load_state(ui.ctx(), response.id) {
                    if let Some(range) = state.cursor.char_range() {
                        let cursor_idx = range.primary.index;
                        current_cursor_idx = cursor_idx;
                        self.cursor_position = cursor_idx;
                        
                        // Check if the character before cursor is '/'
                        if cursor_idx > 0 {
                            let chars: Vec<char> = self.content_buffer.chars().collect();
                            if cursor_idx <= chars.len() && chars.get(cursor_idx - 1) == Some(&'/') {
                                // Check it's at start of line or after whitespace
                                let prev_char = if cursor_idx >= 2 { chars.get(cursor_idx - 2) } else { None };
                                let is_valid_trigger = prev_char.is_none() || prev_char == Some(&'\n') || prev_char.map(|c| c.is_whitespace()) == Some(true);
                                
                                // Also check we are NOT in a code block
                                let is_in_code = self.parser.is_in_code_block(&self.content_buffer, cursor_idx);
                                
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
                    }
                }
                
                // Update node content
                if let Some(node) = graph.node_mut(node_idx) {
                    node.payload_mut().content = self.content_buffer.clone();
                }
                *change_count += 1;
                
                // Handle wikilinks
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                handle_wikilinks(graph, node_idx, &mut self.parser, wikilink_regex, enter_pressed);
            }
        });

        // Slash Menu Popup
        if self.slash_menu_open {
            if let Some(pos) = self.slash_menu_pos {
                let filtered_items: Vec<_> = self.slash_menu_items.iter()
                    .filter(|(_, label, _)| {
                        self.slash_menu_query.is_empty() || 
                        label.to_lowercase().contains(&self.slash_menu_query.to_lowercase())
                    })
                    .cloned()
                    .collect();

                let mut action_nav_down = false;
                let mut action_nav_up = false;
                let mut action_insert = false;
                let mut action_escape = false;
                let mut action_click_idx = None;

                if !filtered_items.is_empty() {
                    egui::Area::new(Id::new("slash_menu"))
                        .fixed_pos(pos + egui::Vec2::new(0.0, 20.0))
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| {
                            Frame::popup(ui.style()).show(ui, |ui| {
                                ui.set_max_width(200.0);
                                ui.set_max_height(300.0);
                                
                                // Handle keyboard navigation
                                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                                    action_nav_down = true;
                                }
                                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                                    action_nav_up = true;
                                }
                                if ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Tab)) {
                                    action_insert = true;
                                }
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    action_escape = true;
                                }

                                ScrollArea::vertical().show(ui, |ui| {
                                    for (i, (icon, label, _)) in filtered_items.iter().enumerate() {
                                        let selected = i == self.slash_menu_selection;
                                        let bg = if selected { ui.visuals().selection.bg_fill } else { Color32::TRANSPARENT };
                                        let fg = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
                                        
                                        let response = ui.allocate_ui(egui::vec2(ui.available_width(), 24.0), |ui| {
                                            ui.painter().rect_filled(ui.max_rect(), 2.0, bg);
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(*icon).color(fg));
                                                ui.label(egui::RichText::new(*label).color(fg));
                                            });
                                        }).response;

                                        if response.clicked() {
                                            action_click_idx = Some(i);
                                        }
                                        
                                        if selected {
                                            response.scroll_to_me(Some(Align::Center));
                                        }
                                    }
                                });
                            });
                        });
                }

                if action_nav_down {
                    self.slash_menu_selection = (self.slash_menu_selection + 1) % filtered_items.len();
                }
                if action_nav_up {
                    if self.slash_menu_selection == 0 {
                        self.slash_menu_selection = filtered_items.len() - 1;
                    } else {
                        self.slash_menu_selection -= 1;
                    }
                }
                if action_escape {
                    self.slash_menu_open = false;
                }
                if let Some(idx) = action_click_idx {
                    self.slash_menu_selection = idx;
                    action_insert = true;
                }
                if action_insert {
                    self.insert_slash_template(node_idx, graph, change_count, wikilink_regex, current_cursor_idx);
                }
            }
        }
    }

    fn insert_slash_template(
        &mut self, 
        node_idx: petgraph::stable_graph::NodeIndex,
        graph: &mut Graph<LogNodeData, LogEdgeData, Directed, u32, LogNode, LogEdge>,
        change_count: &mut usize,
        wikilink_regex: &Regex,
        cursor_idx: usize
    ) {
        // Get the filtered items based on current query
        let filtered_items: Vec<_> = self.slash_menu_items.iter()
            .filter(|(_, item_label, _)| {
                self.slash_menu_query.is_empty() || 
                item_label.to_lowercase().contains(&self.slash_menu_query.to_lowercase())
            })
            .collect();
        
        if let Some((_, _, template)) = filtered_items.get(self.slash_menu_selection) {
            let chars: Vec<char> = self.content_buffer.chars().collect();
            
            // Find the slash backwards from current cursor
            let mut slash_pos = None;
            // Use cursor_idx if valid, otherwise fallback to self.cursor_position or just search backwards from end if query matches
            // But cursor_idx passed from show might be 0 if we didn't get state.
            // If cursor_idx is 0, we might be in trouble. But action_insert happens when menu is open.
            // If menu is open, we likely have a valid cursor position tracked.
            
            let search_end = if cursor_idx > 0 { cursor_idx } else { self.cursor_position };
            
            for i in (0..search_end).rev() {
                if chars.get(i) == Some(&'/') {
                    slash_pos = Some(i);
                    break;
                }
                // Stop if we hit a newline or too far back?
                // Just rely on finding the nearest slash.
            }
            
            if let Some(sp) = slash_pos {
                let before_slash: String = chars.iter().take(sp).collect();
                // We replace everything from slash_pos to search_end (which should be the end of query)
                // But wait, if the user typed query, search_end is after query.
                // So we remove from sp to search_end.
                
                let after_cursor: String = chars.iter().skip(search_end).collect();
                
                // Find where the cursor should be placed (marked by '|' in template)
                let template_str = *template;
                let cursor_marker_pos = template_str.find('|').unwrap_or(template_str.len());
                let template_before_cursor: String = template_str.chars().take(cursor_marker_pos).collect();
                let template_after_cursor: String = template_str.chars().skip(cursor_marker_pos + 1).collect();
                
                // Build new content
                let new_content = format!("{}{}{}{}", before_slash, template_before_cursor, template_after_cursor, after_cursor);
                self.content_buffer = new_content;
                
                // Calculate new cursor position
                self.cursor_position = before_slash.chars().count() + template_before_cursor.chars().count();
                
                // Save to node
                if let Some(node) = graph.node_mut(node_idx) {
                    node.payload_mut().content = self.content_buffer.clone();
                }
                *change_count += 1;
                handle_wikilinks(graph, node_idx, &mut self.parser, wikilink_regex, false);
            }
        }
        
        // Close the menu
        self.slash_menu_open = false;
        self.slash_menu_query.clear();
    }
}
