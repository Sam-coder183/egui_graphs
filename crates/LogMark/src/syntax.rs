use egui_code_editor::Syntax;
use std::collections::BTreeSet;

pub fn markdown() -> Syntax {
    Syntax::new("Markdown")
        .with_comment(">")
        .with_comment_multiline(["<!--", "-->"])
        .with_keywords(BTreeSet::from([
            "true", "false", "null", "fn", "let", "mut", "pub", "struct", "enum", "impl", "use", "mod", "crate", // Rust
            "function", "return", "var", "const", "if", "else", "for", "while", // JS/Lua
            "local", "then", "end", "do", "repeat", "until", // Lua
        ]))
        .with_types(BTreeSet::from([
            "String", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64", "bool", "usize", "isize", "char", "str", // Rust
            "Option", "Result", "Vec", "Box", "Rc", "Arc", // Rust
            "table", "string", "number", "boolean", "nil", // Lua
        ]))
        .with_special(BTreeSet::from([
            "TODO", "FIXME", "NOTE", "BUG", "HACK",
        ]))
}
