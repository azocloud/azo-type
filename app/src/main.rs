pub mod app;
pub mod compiler;
pub mod highlight;
pub mod vfs;
pub mod world;

use std::path::PathBuf;

use app::TypstIdeApp;

#[cfg(not(target_arch = "wasm32"))]
fn parse_screenshot_arg() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let mut idx = 1;
    while idx < args.len() {
        if args[idx] == "--screenshot" {
            if let Some(path_str) = args.get(idx + 1) {
                return Some(PathBuf::from(path_str));
            }
        } else if let Some(rest) = args[idx].strip_prefix("--screenshot=") {
            return Some(PathBuf::from(rest));
        }
        idx += 1;
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let screenshot_path = parse_screenshot_arg();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Typst Studio — Cross-Platform & Web Typst IDE")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([800.0, 520.0]);

    if screenshot_path.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    if screenshot_path.is_some() {
        native_options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "Typst Studio",
        native_options,
        Box::new(move |cc| Ok(Box::new(TypstIdeApp::new(cc, screenshot_path)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast as _;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find #the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#the_canvas_id was not an HtmlCanvasElement");

        let web_options = eframe::WebOptions::default();
        let _ = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(TypstIdeApp::new(cc, None)))),
            )
            .await;
    });
}
