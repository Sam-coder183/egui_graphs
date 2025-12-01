// Native-only binary - not compiled for WASM
#![cfg(not(target_arch = "wasm32"))]

use LogMark::LogMarkApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let native_options = NativeOptions::default();
    eframe::run_native(
        "LogMark",
        native_options,
        Box::new(|cc| Ok(Box::new(LogMarkApp::new(cc)))),
    )
}
