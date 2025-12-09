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
    ThreeD,
}

impl VisualizationMode {
    pub fn label(&self) -> &str {
        match self {
            VisualizationMode::TwoD => "2D View",
            VisualizationMode::ThreeD => "3D View",
        }
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
