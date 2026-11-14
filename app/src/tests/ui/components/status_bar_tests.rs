//! Headless `egui_kittest` tests for the bottom status bar component.

use std::path::Path;

use egui_kittest::{kittest::Queryable, Harness};
use rust_i18n::t;

use super::*;

#[test]
fn status_bar_renders_diagnostics_collab_and_opens_compiler_settings_on_click() {
    let open_settings = false;
    let engine_btn_label =
        t!("status.typst_version", locale = "en", version = "0.15.1").to_string();

    let mut harness = Harness::new_state(
        |ctx, open_settings: &mut bool| {
            let out = render_status_bar(
                ctx,
                StatusBarParams {
                    dark_mode: true,
                    compile_in_progress: false,
                    pending_compile: true,
                    error_count: 1,
                    warn_count: 2,
                    compile_duration_ms: 14.2,
                    main_file: Path::new("main.typ"),
                    page_count: 3,
                    active_typst_version: "0.15.1",
                    downloading_typst_version: None,
                    typst_fallback_active: true,
                    collab_active: true,
                    collab_is_host: true,
                    collab_peer_count: 2,
                    status_notice: "Ready",
                    active_buffer: Some(StatusBarBufferInfo {
                        rel_path: Path::new("main.typ"),
                        dirty: true,
                        line_count: 42,
                        size_bytes: 512,
                    }),
                },
            );
            *open_settings |= out.open_compiler_settings;
        },
        open_settings,
    );

    harness.get_by_label(&engine_btn_label).click();
    harness.run();
    assert!(*harness.state());
}

#[test]
fn status_bar_renders_compiling_and_downloading_light_mode() {
    let mut harness = Harness::new(|ctx| {
        render_status_bar(
            ctx,
            StatusBarParams {
                dark_mode: false,
                compile_in_progress: true,
                pending_compile: false,
                error_count: 0,
                warn_count: 0,
                compile_duration_ms: 0.0,
                main_file: Path::new("main.typ"),
                page_count: 0,
                active_typst_version: "0.15.1",
                downloading_typst_version: Some("0.14.2"),
                typst_fallback_active: false,
                collab_active: true,
                collab_is_host: false,
                collab_peer_count: 1,
                status_notice: "Compiling...",
                active_buffer: None,
            },
        );
    });
    harness.step();
}
