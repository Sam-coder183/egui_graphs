#[derive(PartialEq)]
pub enum AppLayout {
    Random,
    Force,
    Hierarchical,
}

#[derive(Clone, Copy, PartialEq)]
pub enum QuickTemplate {
    Note,
    Heading,
    Task,
}

#[derive(PartialEq, Clone, Copy)]
pub enum AppTab {
    Graph,
    Preview,
}

#[derive(PartialEq, Clone, Copy)]
pub enum VisualizationMode {
    TwoD,
    Journal,
    CodeAnalysis,
}

impl VisualizationMode {
    pub fn label(&self) -> &str {
        match self {
            VisualizationMode::TwoD => "Documentation View 📝",
            VisualizationMode::Journal => "Journal View 📔",
            VisualizationMode::CodeAnalysis => "Code Analysis",
        }
    }

    pub fn is_doc_view(&self) -> bool {
        matches!(self, VisualizationMode::TwoD | VisualizationMode::Journal)
    }

    pub fn is_note_mode(&self) -> bool {
        matches!(self, VisualizationMode::Journal)
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum SidebarTab {
    Edit,
    Preview,
    Lua,
}

#[derive(PartialEq, Clone, Copy)]
pub enum EditorMode {
    View,
    Edit,
    Split,
    Hybrid, // Logseq-style
}
