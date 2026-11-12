use typst_ide::app::TypstIdeApp;

fn parse_cli_args() -> (Option<std::path::PathBuf>, Option<std::path::PathBuf>) {
    let args: Vec<String> = std::env::args().collect();
    typst_ide::parse_cli_args_from(&args)
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
