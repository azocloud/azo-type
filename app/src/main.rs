pub mod app;
pub mod compiler;
pub mod highlight;
pub mod project;
pub mod world;

use std::path::PathBuf;

use app::TypstIdeApp;

fn parse_cli_args() -> (Option<PathBuf>, Option<PathBuf>) {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot_path = None;
    let mut folder_path = None;

    let mut idx = 1;
    while idx < args.len() {
        if args[idx] == "--screenshot" {
            if let Some(path_str) = args.get(idx + 1) {
                screenshot_path = Some(PathBuf::from(path_str));
                idx += 2;
                continue;
            }
        } else if let Some(rest) = args[idx].strip_prefix("--screenshot=") {
            screenshot_path = Some(PathBuf::from(rest));
            idx += 1;
            continue;
        } else if !args[idx].starts_with('-') && folder_path.is_none() {
            let candidate = PathBuf::from(&args[idx]);
            if candidate.is_dir() {
                folder_path = Some(candidate);
            }
        }
        idx += 1;
    }

    (folder_path, screenshot_path)
}

fn main() -> eframe::Result<()> {
    let (initial_folder, screenshot_path) = parse_cli_args();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Typst Studio — Native Desktop Typst IDE")
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
        Box::new(move |cc| {
            Ok(Box::new(TypstIdeApp::new(
                cc,
                initial_folder,
                screenshot_path,
            )))
        }),
    )
}
