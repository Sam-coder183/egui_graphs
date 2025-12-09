// Native-only binary - not compiled for WASM
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use logmark::LogMarkApp;
    use eframe::NativeOptions;

    let native_options = NativeOptions::default();
    eframe::run_native(
        "LogMark",
        native_options,
        Box::new(|cc| Ok(Box::new(LogMarkApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
