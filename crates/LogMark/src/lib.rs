pub mod app;
pub mod graph;
pub mod syntax;
pub mod parser;
pub mod types;
pub mod utils;
pub mod persistence;
pub mod lua;
pub mod actions;
pub mod ui;
pub mod analyzer;

pub use app::LogMarkApp;

use std::sync::RwLock;

pub struct GraphSettings {
    pub font_size_edge_label: f32,
}

pub static GRAPH_SETTINGS: RwLock<GraphSettings> = RwLock::new(GraphSettings {
    font_size_edge_label: 14.0,
});

// WASM entry point
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::LogMarkApp;
    use wasm_bindgen::prelude::*;

    /// Our handle to the web app from JavaScript.
    #[derive(Clone)]
    #[wasm_bindgen]
    pub struct WebHandle {
        runner: eframe::WebRunner,
    }

    #[wasm_bindgen]
    impl WebHandle {
        /// Installs a panic hook, then returns.
        #[allow(clippy::new_without_default)]
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            // Redirect [`log`] message to `console.log` and friends:
            eframe::WebLogger::init(log::LevelFilter::Debug).ok();
            
            Self {
                runner: eframe::WebRunner::new(),
            }
        }

        /// Call this once from JavaScript to start your app.
        #[wasm_bindgen]
        pub async fn start(
            &self,
            canvas: web_sys::HtmlCanvasElement,
        ) -> Result<(), wasm_bindgen::JsValue> {
            self.runner
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(LogMarkApp::new(cc)))),
                )
                .await
        }

        /// Shut down eframe and clean up resources.
        #[wasm_bindgen]
        pub fn destroy(&self) {
            self.runner.destroy();
        }

        /// The JavaScript can check whether or not your app has crashed:
        #[wasm_bindgen]
        pub fn has_panicked(&self) -> bool {
            self.runner.has_panicked()
        }

        #[wasm_bindgen]
        pub fn panic_message(&self) -> Option<String> {
            self.runner.panic_summary().map(|s| s.message())
        }

        #[wasm_bindgen]
        pub fn panic_callstack(&self) -> Option<String> {
            self.runner.panic_summary().map(|s| s.callstack())
        }
    }
}

