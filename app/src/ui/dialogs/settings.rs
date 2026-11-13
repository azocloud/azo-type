//! Settings modal dialog (`File > Settings`) managing editor, appearance, and compilation preferences.

use std::path::Path;

use egui::{
    CentralPanel, Context, DragValue, Frame, Margin, RichText, Ui, ViewportBuilder, ViewportClass,
    ViewportId, Window,
};
use rust_i18n::t;

use crate::core::engine::all_selectable_versions;
use crate::core::settings::{IdeSettings, SettingsTab, ThemePreference};
use crate::engine::{DownloadProgress, DownloadStage, EngineInstallState, EngineManager};
use crate::ui::theme::{apply_theme, ThemePalette};

/// Actions and side effects requested by user interaction within the Settings dialog.
#[derive(Debug, Default)]
pub struct SettingsModalOutput {
    pub open_folder_requested: bool,
    pub theme_changed: bool,
    pub rescan_needed: bool,
    pub dpi_changed: bool,
    pub language_changed: Option<String>,
    pub engine_changed: bool,
    pub download_typst_version: Option<String>,
    pub remove_typst_version: Option<String>,
}

/// Renders the Settings sub-window (embedded or native OS viewport).
pub fn render_settings_dialog(
    ctx: &Context,
    open: &mut bool,
    settings: &mut IdeSettings,
    language: &str,
    active_tab: &mut SettingsTab,
    project_root: &Path,
    engine_manager: &EngineManager,
) -> SettingsModalOutput {
    if !*open {
        return SettingsModalOutput::default();
    }

    let mut is_open = *open;
    let mut output = SettingsModalOutput::default();

    let dark_mode = settings.dark_mode;
    let palette = ThemePalette::for_mode(dark_mode);
    let window_bg = palette.chrome_sidebar;

    let viewport_id = ViewportId::from_hash_of("azo_type_settings_os_window");
    let builder = crate::platform::configure_viewport_builder(
        ViewportBuilder::default()
            .with_title(t!("settings.window_title"))
            .with_inner_size([540.0, 440.0])
            .with_min_inner_size([440.0, 340.0]),
    );

    ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
        let mut render_body = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    active_tab,
                    SettingsTab::Appearance,
                    RichText::new(t!("settings.tab_appearance")).strong(),
                );
                ui.selectable_value(
                    active_tab,
                    SettingsTab::EditorAndDisk,
                    RichText::new(t!("settings.tab_editor")).strong(),
                );
                ui.selectable_value(
                    active_tab,
                    SettingsTab::CompilerAndPreview,
                    RichText::new(t!("settings.tab_compiler")).strong(),
                );
            });
            ui.separator();
            ui.add_space(6.0);

            match active_tab {
                SettingsTab::Appearance => {
                    ui.heading(t!("settings.heading_appearance"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.language_label"));
                        let current_name = crate::core::locale_native_name(language);
                        egui::ComboBox::from_id_salt("settings_language_combo")
                            .selected_text(current_name)
                            .show_ui(ui, |ui| {
                                for loc in rust_i18n::available_locales!() {
                                    let name = crate::core::locale_native_name(loc);
                                    if ui.selectable_label(loc == language, name).clicked() {
                                        output.language_changed = Some(loc.to_string());
                                    }
                                }
                            });
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.theme_label"));
                        let selected_theme_label =
                            theme_preference_label(settings.theme_preference);
                        egui::ComboBox::from_id_salt("settings_theme_combo")
                            .selected_text(selected_theme_label)
                            .show_ui(ui, |ui| {
                                for pref in ThemePreference::ALL {
                                    let label = theme_preference_label(pref);
                                    if ui
                                        .selectable_label(settings.theme_preference == pref, label)
                                        .clicked()
                                    {
                                        settings.theme_preference = pref;
                                        let next_dark =
                                            crate::platform::resolve_effective_dark_mode(ctx, pref);
                                        if next_dark != settings.dark_mode {
                                            settings.dark_mode = next_dark;
                                            output.theme_changed = true;
                                        }
                                    }
                                }
                            });
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.font_size_label"));
                        ui.add(
                            DragValue::new(&mut settings.editor_font_size)
                                .range(10.0..=22.0)
                                .speed(0.5)
                                .fixed_decimals(1)
                                .suffix(" pt"),
                        );
                    });

                    ui.add_space(8.0);
                    ui.checkbox(&mut settings.word_wrap, t!("settings.word_wrap_label"));
                    ui.checkbox(
                        &mut settings.show_line_numbers,
                        t!("settings.line_numbers_label"),
                    );
                    ui.checkbox(
                        &mut settings.syntax_highlighting,
                        t!("settings.syntax_highlighting_label"),
                    );
                }
                SettingsTab::EditorAndDisk => {
                    ui.heading(t!("settings.heading_editor"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            t!("settings.project_folder_label"),
                            project_root.display()
                        ))
                        .monospace()
                        .small(),
                    );
                    ui.add_space(6.0);
                    ui.checkbox(&mut settings.word_wrap, t!("settings.word_wrap_label"));
                    ui.checkbox(
                        &mut settings.auto_save_on_edit,
                        t!("settings.auto_save_label"),
                    );
                    if ui
                        .checkbox(
                            &mut settings.show_hidden_files,
                            t!("settings.show_hidden_label"),
                        )
                        .changed()
                    {
                        output.rescan_needed = true;
                    }
                    ui.add_space(8.0);
                    if ui.button(t!("settings.choose_folder_btn")).clicked() {
                        output.open_folder_requested = true;
                    }
                }
                SettingsTab::CompilerAndPreview => {
                    ui.heading(t!("settings.heading_compiler"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.typst_version_label"));
                        let selected_state = engine_manager.install_state(&settings.typst_version);
                        let selected_label =
                            format_version_label(&settings.typst_version, &selected_state);
                        egui::ComboBox::from_id_salt("typst_version_selector")
                            .selected_text(selected_label)
                            .show_ui(ui, |ui| {
                                for ver in all_selectable_versions() {
                                    let st = engine_manager.install_state(ver);
                                    let label = format_version_label(ver, &st);
                                    if ui
                                        .selectable_value(
                                            &mut settings.typst_version,
                                            ver.to_string(),
                                            label,
                                        )
                                        .changed()
                                    {
                                        output.engine_changed = true;
                                    }
                                }
                            });

                        match selected_state {
                            EngineInstallState::BuiltIn => {
                                ui.label(
                                    RichText::new(t!("settings.typst_builtin_badge"))
                                        .small()
                                        .color(palette.text_muted),
                                );
                            }
                            EngineInstallState::NotInstalled => {
                                if ui.button(t!("settings.typst_download_btn")).clicked() {
                                    output.download_typst_version =
                                        Some(settings.typst_version.clone());
                                }
                            }
                            EngineInstallState::Downloading => {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(t!("settings.typst_downloading_btn")),
                                );
                            }
                            EngineInstallState::Installed => {
                                if ui.button(t!("settings.typst_remove_btn")).clicked() {
                                    output.remove_typst_version =
                                        Some(settings.typst_version.clone());
                                }
                            }
                            EngineInstallState::UnsupportedPlatform => {}
                        }
                    });
                    if let Some(prog) = engine_manager
                        .download_progress(&settings.typst_version)
                        .or_else(|| engine_manager.any_download_progress())
                    {
                        ui.add_space(6.0);
                        render_download_progress(ui, &prog);
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(100));
                    }
                    if let Some(err_msg) = engine_manager.last_error() {
                        ui.add_space(4.0);
                        ui.label(RichText::new(err_msg).small().color(palette.syn_number));
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.checkbox(
                        &mut settings.auto_compile,
                        t!("settings.auto_compile_label"),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.debounce_label"));
                        ui.add(
                            DragValue::new(&mut settings.debounce_ms)
                                .range(50..=800)
                                .speed(5)
                                .suffix(" ms"),
                        );
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(t!("settings.dpi_scale_label"));
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                1.5,
                                t!("settings.dpi_fast"),
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                2.0,
                                t!("settings.dpi_crisp"),
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.render_dpi_scale,
                                3.0,
                                t!("settings.dpi_hidpi"),
                            )
                            .changed()
                        {
                            output.dpi_changed = true;
                        }
                    });
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.checkbox(
                        &mut settings.preview_sync_selection,
                        t!("settings.preview_sync_selection_label"),
                    );
                    ui.checkbox(
                        &mut settings.preview_pan_mode,
                        t!("settings.preview_pan_mode_label"),
                    );
                }
            }

            ui.add_space(14.0);
            ui.separator();
            if ui.button(t!("settings.reset_defaults")).clicked() {
                let prev_dark = settings.dark_mode;
                *settings = IdeSettings::default();
                settings.dark_mode =
                    crate::platform::resolve_effective_dark_mode(ctx, settings.theme_preference);
                if prev_dark != settings.dark_mode {
                    output.theme_changed = true;
                }
                output.dpi_changed = true;
                output.engine_changed = true;
            }
        };

        if class == ViewportClass::Embedded {
            Window::new(t!("settings.window_title"))
                .open(&mut is_open)
                .resizable(true)
                .collapsible(false)
                .default_width(480.0)
                .default_height(360.0)
                .show(ctx, |ui| render_body(ui));
        } else {
            apply_theme(ctx, dark_mode);
            if ctx.input(|i| i.viewport().close_requested()) {
                is_open = false;
            }
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(window_bg)
                        .inner_margin(Margin::symmetric(14, 12)),
                )
                .show(ctx, |ui| render_body(ui));
        }
    });

    *open = is_open;
    output
}

fn theme_preference_label(pref: ThemePreference) -> String {
    match pref {
        ThemePreference::Light => t!("settings.theme_light").to_string(),
        ThemePreference::Dark => t!("settings.theme_dark").to_string(),
        ThemePreference::SystemDefault => t!("settings.theme_system").to_string(),
    }
}

fn format_version_label(version: &str, state: &EngineInstallState) -> String {
    match state {
        EngineInstallState::BuiltIn => t!("settings.typst_version_builtin", version = version),
        EngineInstallState::Installed => t!("settings.typst_version_installed", version = version),
        EngineInstallState::NotInstalled => {
            t!("settings.typst_version_not_installed", version = version)
        }
        EngineInstallState::Downloading => {
            t!("settings.typst_version_downloading", version = version)
        }
        EngineInstallState::UnsupportedPlatform => {
            t!("settings.typst_version_unsupported", version = version)
        }
    }
    .to_string()
}

fn render_download_progress(ui: &mut Ui, prog: &DownloadProgress) {
    let elapsed = format!("{:.1}", prog.elapsed_secs());
    let dl_mb = format!("{:.1}", prog.downloaded_bytes as f64 / (1024.0 * 1024.0));
    let speed_mb = format!("{:.1}", prog.speed_bytes_per_sec() / (1024.0 * 1024.0));

    let (fraction, status_text) = match prog.stage {
        DownloadStage::Connecting => (
            0.05,
            t!(
                "settings.typst_progress_connecting",
                version = prog.version.as_str(),
                elapsed = elapsed.as_str()
            )
            .to_string(),
        ),
        DownloadStage::Downloading => {
            let frac = prog.fraction().unwrap_or(0.25);
            let msg = match (prog.total_bytes, prog.eta_secs()) {
                (Some(total), Some(eta)) => t!(
                    "settings.typst_progress_downloading_eta",
                    version = prog.version.as_str(),
                    downloaded = dl_mb.as_str(),
                    total = format!("{:.1}", total as f64 / (1024.0 * 1024.0)),
                    speed = speed_mb.as_str(),
                    eta = format!("{:.0}", eta.ceil())
                )
                .to_string(),
                (Some(total), None) => t!(
                    "settings.typst_progress_downloading",
                    version = prog.version.as_str(),
                    downloaded = dl_mb.as_str(),
                    total = format!("{:.1}", total as f64 / (1024.0 * 1024.0)),
                    speed = speed_mb.as_str(),
                    elapsed = elapsed.as_str()
                )
                .to_string(),
                (None, _) => t!(
                    "settings.typst_progress_downloading_unknown",
                    version = prog.version.as_str(),
                    downloaded = dl_mb.as_str(),
                    speed = speed_mb.as_str(),
                    elapsed = elapsed.as_str()
                )
                .to_string(),
            };
            (frac, msg)
        }
        DownloadStage::VerifyingAndExtracting => (
            1.0,
            t!(
                "settings.typst_progress_verifying",
                version = prog.version.as_str(),
                elapsed = elapsed.as_str()
            )
            .to_string(),
        ),
    };

    ui.add(
        egui::ProgressBar::new(fraction)
            .animate(true)
            .text(status_text),
    );
}
