use egui::{Context, Window};
pub fn show_help_window(ctx: &Context, open: &mut bool) {
    if *open {
        let mut is_open = true;
        Window::new("Help & Shortcuts")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Keyboard Shortcuts");
                egui::Grid::new("shortcuts_grid").striped(true).show(ui, |ui| {
                    ui.label("Search"); ui.label("Ctrl + F"); ui.end_row();
                    ui.label("Quick Add"); ui.label("Ctrl + Enter"); ui.end_row();
                    ui.label("Zen Mode"); ui.label("Ctrl + Z"); ui.end_row();
                    ui.label("Save"); ui.label("Ctrl + S"); ui.end_row();
                    ui.label("Open"); ui.label("Ctrl + O"); ui.end_row();
                    ui.label("New"); ui.label("Ctrl + N"); ui.end_row();
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
        if !is_open {
            *open = false;
        }
    }
}
