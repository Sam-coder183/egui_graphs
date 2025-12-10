use std::path::{Path, PathBuf};
use std::fs;
use walkdir::WalkDir;
use tree_sitter::{Parser, Query, QueryCursor};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub imports: Vec<String>,
}

pub struct ProjectAnalyzer {
    rust_query: Query,
    python_query: Query,
    rust_imports_query: Query,
    python_imports_query: Query,
}

impl ProjectAnalyzer {
    pub fn new() -> Self {
        let rust_query_str = r#"
            (struct_item name: (type_identifier) @struct)
            (impl_item type: (type_identifier) @impl)
            (function_item name: (identifier) @fn)
            (trait_item name: (type_identifier) @trait)
            (enum_item name: (type_identifier) @enum)
        "#;

        let rust_imports_str = r#"
            (use_declaration argument: (_) @import)
            (mod_item name: (identifier) @mod)
        "#;

        let python_query_str = r#"
            (class_definition name: (identifier) @class)
            (function_definition name: (identifier) @fn)
        "#;

        let python_imports_str = r#"
            (import_statement name: (_) @import)
            (import_from_statement module_name: (_) @from_import)
        "#;

        let rust_query = Query::new(tree_sitter_rust::language(), rust_query_str).expect("Invalid Rust Query");
        let rust_imports_query = Query::new(tree_sitter_rust::language(), rust_imports_str).expect("Invalid Rust Imports Query");
        
        let python_query = Query::new(tree_sitter_python::language(), python_query_str).expect("Invalid Python Query");
        let python_imports_query = Query::new(tree_sitter_python::language(), python_imports_str).expect("Invalid Python Imports Query");

        Self {
            rust_query,
            python_query,
            rust_imports_query,
            python_imports_query,
        }
    }

    pub fn analyze_path(&self, path: &Path) -> Vec<FileNode> {
        let mut files = Vec::new();

        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    match ext {
                        "rs" => {
                            if let Some(node) = self.analyze_rust_file(path) {
                                files.push(node);
                            }
                        },
                        "py" => {
                            if let Some(node) = self.analyze_python_file(path) {
                                files.push(node);
                            }
                        },
                        _ => {}
                    }
                }
            }
        }

        files
    }

    fn analyze_rust_file(&self, path: &Path) -> Option<FileNode> {
        let source_code = fs::read_to_string(path).ok()?;
        let mut parser = Parser::new();
        parser.set_language(tree_sitter_rust::language()).ok()?;
        let tree = parser.parse(&source_code, None)?;
        
        let mut content = String::new();
        content.push_str(&format!("# File: {}\n\n", path.file_name()?.to_string_lossy()));

        let mut imports = Vec::new();
        let mut cursor = QueryCursor::new();

        // Extract Structure
        content.push_str("## Structure\n\n");
        for match_ in cursor.matches(&self.rust_query, tree.root_node(), |node| &source_code.as_bytes()[node.start_byte()..node.end_byte()]) {
            for capture in match_.captures {
                let capture_name = self.rust_query.capture_names()[capture.index as usize].as_str();
                let node = capture.node;
                let text = &source_code[node.start_byte()..node.end_byte()];
                
                match capture_name {
                    "struct" => content.push_str(&format!("- **Struct**: `{}`\n", text)),
                    "impl" => content.push_str(&format!("- **Impl**: `{}`\n", text)),
                    "fn" => content.push_str(&format!("- **Fn**: `{}`\n", text)),
                    "trait" => content.push_str(&format!("- **Trait**: `{}`\n", text)),
                    "enum" => content.push_str(&format!("- **Enum**: `{}`\n", text)),
                    _ => {}
                }
            }
        }

        // Extract Imports
        let mut cursor = QueryCursor::new();
        for match_ in cursor.matches(&self.rust_imports_query, tree.root_node(), |node| &source_code.as_bytes()[node.start_byte()..node.end_byte()]) {
            for capture in match_.captures {
                let capture_name = self.rust_imports_query.capture_names()[capture.index as usize].as_str();
                let node = capture.node;
                let text = &source_code[node.start_byte()..node.end_byte()];
                
                if capture_name == "mod" {
                    imports.push(text.to_string());
                } else {
                    let parts: Vec<&str> = text.split("::").collect();
                    for part in parts {
                        let clean_part = part.trim_matches(|c| c == '{' || c == '}' || c == ' ' || c == ';');
                        if !clean_part.is_empty() && clean_part != "crate" && clean_part != "std" && clean_part != "self" && clean_part != "super" {
                            imports.push(clean_part.to_string());
                        }
                    }
                }
            }
        }

        Some(FileNode {
            path: path.to_path_buf(),
            name: path.file_name()?.to_string_lossy().to_string(),
            content,
            imports,
        })
    }

    fn analyze_python_file(&self, path: &Path) -> Option<FileNode> {
        let source_code = fs::read_to_string(path).ok()?;
        let mut parser = Parser::new();
        parser.set_language(tree_sitter_python::language()).ok()?;
        let tree = parser.parse(&source_code, None)?;
        
        let mut content = String::new();
        content.push_str(&format!("# File: {}\n\n", path.file_name()?.to_string_lossy()));

        let mut imports = Vec::new();
        let mut cursor = QueryCursor::new();

        // Extract Structure
        content.push_str("## Structure\n\n");
        for match_ in cursor.matches(&self.python_query, tree.root_node(), |node| &source_code.as_bytes()[node.start_byte()..node.end_byte()]) {
            for capture in match_.captures {
                let capture_name = self.python_query.capture_names()[capture.index as usize].as_str();
                let node = capture.node;
                let text = &source_code[node.start_byte()..node.end_byte()];
                
                match capture_name {
                    "class" => content.push_str(&format!("- **Class**: `{}`\n", text)),
                    "fn" => content.push_str(&format!("- **Fn**: `{}`\n", text)),
                    _ => {}
                }
            }
        }

        // Extract Imports
        let mut cursor = QueryCursor::new();
        for match_ in cursor.matches(&self.python_imports_query, tree.root_node(), |node| &source_code.as_bytes()[node.start_byte()..node.end_byte()]) {
            for capture in match_.captures {
                let node = capture.node;
                let text = &source_code[node.start_byte()..node.end_byte()];
                
                let parts: Vec<&str> = text.split('.').collect();
                for part in parts {
                     let clean_part = part.trim();
                     if !clean_part.is_empty() {
                         imports.push(clean_part.to_string());
                     }
                }
            }
        }

        Some(FileNode {
            path: path.to_path_buf(),
            name: path.file_name()?.to_string_lossy().to_string(),
            content,
            imports,
        })
    }
}
