use tree_sitter::{Parser, Node, Tree};
use tree_sitter_markdown;
use std::ops::Range;

pub struct MarkdownParser {
    parser: Parser,
}

impl MarkdownParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser.set_language(tree_sitter_markdown::language()).expect("Error loading Markdown grammar");
        Self { parser }
    }

    pub fn parse(&mut self, text: &str) -> Tree {
        self.parser.parse(text, None).expect("Error parsing text")
    }

    /// Returns true if the given byte position is inside a code block or inline code
    pub fn is_in_code_block(&mut self, text: &str, byte_pos: usize) -> bool {
        let tree = self.parse(text);
        let root = tree.root_node();
        
        let mut cursor = root.walk();
        let mut node = root;
        
        // Find the smallest node containing the position
        while let Some(child) = node.named_children(&mut cursor).find(|n| n.byte_range().contains(&byte_pos)) {
            node = child;
        }

        // Check node type
        let kind = node.kind();
        kind == "fenced_code_block" || kind == "code_span" || kind == "indented_code_block"
    }

    /// Returns a list of ranges in the text that are NOT code blocks.
    /// This is useful for running regexes only on "safe" text (e.g. for wikilinks).
    pub fn get_safe_ranges(&mut self, text: &str) -> Vec<Range<usize>> {
        let tree = self.parse(text);
        let root = tree.root_node();
        let mut ranges = Vec::new();
        let text_len = text.len();

        // Simple approach: Traverse and collect ranges that are NOT code
        // A better approach might be to collect code ranges and invert them.
        
        let mut code_ranges = Vec::new();
        self.collect_code_ranges(root, &mut code_ranges);
        
        // Sort and merge code ranges
        code_ranges.sort_by_key(|r| r.start);
        
        let mut last_end = 0;
        for range in code_ranges {
            if range.start > last_end {
                ranges.push(last_end..range.start);
            }
            last_end = last_end.max(range.end);
        }
        
        if last_end < text_len {
            ranges.push(last_end..text_len);
        }

        ranges
    }

    fn collect_code_ranges(&self, node: Node, ranges: &mut Vec<Range<usize>>) {
        let kind = node.kind();
        if kind == "fenced_code_block" || kind == "code_span" || kind == "indented_code_block" {
            ranges.push(node.byte_range());
            // Don't recurse into code blocks
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_code_ranges(child, ranges);
        }
    }
}
