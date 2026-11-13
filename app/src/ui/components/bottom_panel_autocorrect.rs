//! Auto-Correction & Language Analysis tab inside the collapsible bottom workspace panel.

use std::path::Path;

use egui::{Color32, ProgressBar, RichText, Ui};
use rust_i18n::t;

use crate::core::spellcheck::DiagnosticCategory;
use crate::language_check::LanguageCheckHost;
use crate::ui::components::bottom_panel::BottomPanelOutput;
use crate::ui::components::editor_diagnostics::diagnostic_color;
use crate::ui::theme::ThemePalette;

/// Renders controls, download prompts, and interactive diagnostics inside the Auto-Correction tab.
#[allow(clippy::too_many_arguments)]
pub fn render_autocorrect_tab(
    ui: &mut Ui,
    dark_mode: bool,
    palette: &ThemePalette,
    active_file: &Path,
    is_active_typst: bool,
    spell_check_enabled: &mut bool,
    auto_spell_check: &mut bool,
    language_check: &LanguageCheckHost,
    output: &mut BottomPanelOutput,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t!("bottom_panel.autocorrect_heading"))
                .small()
                .strong(),
        );
        ui.separator();
        if ui
            .checkbox(
                spell_check_enabled,
                t!("bottom_panel.autocorrect_enable_label"),
            )
            .changed()
        {
            output.check_language_now = true;
        }
        ui.add_enabled_ui(*spell_check_enabled, |ui| {
            ui.checkbox(auto_spell_check, t!("bottom_panel.autocorrect_auto_label"));
            let check_btn_label = if language_check.checking {
                t!("bottom_panel.autocorrect_checking")
            } else {
                t!("bottom_panel.autocorrect_check_now")
            };
            if ui
                .add_enabled(!language_check.checking, egui::Button::new(check_btn_label))
                .clicked()
            {
                output.check_language_now = true;
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button(t!("bottom_panel.autocorrect_settings_btn"))
                .clicked()
            {
                output.open_spell_settings = true;
            }
        });
    });

    ui.add_space(4.0);
    let shown_locale = if language_check.effective_lang.is_empty() {
        "auto".to_string()
    } else {
        language_check.effective_lang.clone()
    };
    ui.label(
        RichText::new(t!(
            "bottom_panel.autocorrect_active_doc",
            file = active_file.display().to_string(),
            locale = shown_locale
        ))
        .small()
        .monospace()
        .color(palette.text_muted),
    );
    ui.add_space(4.0);

    if let Some(progress) = language_check.dict_manager.any_download_progress() {
        ui.add(ProgressBar::new(progress.fraction()).show_percentage());
        let dl_mb = progress.downloaded_bytes as f64 / (1024.0 * 1024.0);
        let tot_mb = (progress.total_bytes as f64 / (1024.0 * 1024.0)).max(0.01);
        ui.label(
            RichText::new(t!(
                "settings.spell_download_progress",
                lang = progress.lang_code,
                file = progress.current_filename,
                idx = progress.current_file_index,
                total = progress.total_files,
                downloaded = format!("{dl_mb:.1}"),
                size = format!("{tot_mb:.1}")
            ))
            .small()
            .color(palette.text_muted),
        );
        ui.add_space(4.0);
    }

    if let Some(err) = language_check.dict_manager.last_error() {
        ui.label(
            RichText::new(err)
                .small()
                .color(Color32::from_rgb(220, 60, 60)),
        );
    }

    if !*spell_check_enabled {
        ui.label(
            RichText::new(t!("bottom_panel.autocorrect_disabled_hint"))
                .small()
                .color(palette.text_muted),
        );
        return;
    }

    if !is_active_typst {
        ui.label(
            RichText::new(t!("bottom_panel.autocorrect_non_typst_hint"))
                .small()
                .color(palette.text_muted),
        );
        return;
    }

    if let Some(missing_lang) = &language_check.missing_tier1_pack {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(t!(
                    "bottom_panel.autocorrect_missing_pack",
                    lang = missing_lang
                ))
                .small()
                .color(palette.syn_operator),
            );
            if language_check
                .dict_manager
                .download_progress(missing_lang)
                .is_none()
                && ui
                    .button(t!(
                        "bottom_panel.autocorrect_download_pack_btn",
                        lang = missing_lang
                    ))
                    .clicked()
            {
                output.download_dictionary_lang = Some(missing_lang.clone());
            }
        });
        return;
    }

    if let Some(warn) = &language_check.last_warning {
        ui.label(RichText::new(warn).small().color(palette.syn_operator));
    }

    if language_check.diagnostics.is_empty() {
        ui.label(
            RichText::new(t!(
                "bottom_panel.autocorrect_no_issues",
                ms = format!("{:.1}", language_check.last_duration_ms)
            ))
            .small()
            .color(palette.syn_string),
        );
        return;
    }

    ui.label(
        RichText::new(t!(
            "bottom_panel.autocorrect_summary",
            count = language_check.diagnostics.len(),
            lang = language_check.effective_lang,
            ms = format!("{:.1}", language_check.last_duration_ms)
        ))
        .small()
        .color(palette.text_muted),
    );
    ui.add_space(4.0);

    for (idx, diag) in language_check.diagnostics.iter().enumerate() {
        ui.push_id(("autocorrect_diag", idx), |ui| {
            ui.horizontal_wrapped(|ui| {
                let badge = match diag.category {
                    DiagnosticCategory::Spelling => t!("bottom_panel.autocorrect_spelling_badge"),
                    DiagnosticCategory::Grammar => t!("bottom_panel.autocorrect_grammar_badge"),
                    DiagnosticCategory::Typography => {
                        t!("bottom_panel.autocorrect_typography_badge")
                    }
                };
                let badge_color = diagnostic_color(diag.category, dark_mode);
                ui.label(
                    RichText::new(format!("[{badge}]"))
                        .monospace()
                        .small()
                        .strong()
                        .color(badge_color),
                );

                if ui
                    .small_button(format!("L{}:{}", diag.line, diag.column))
                    .on_hover_text(t!(
                        "bottom_panel.autocorrect_jump_tooltip",
                        line = diag.line,
                        col = diag.column
                    ))
                    .clicked()
                {
                    output.jump_to_char = Some(diag.start_char);
                }

                if !diag.excerpt.is_empty() {
                    ui.label(
                        RichText::new(format!("“{}”", diag.excerpt))
                            .monospace()
                            .small()
                            .strong(),
                    );
                }
                ui.label(
                    RichText::new(&diag.message)
                        .small()
                        .color(palette.text_muted),
                );

                for sugg in diag.suggestions.iter().take(4) {
                    if ui.small_button(format!("→ {sugg}")).clicked() {
                        output.apply_suggestion =
                            Some((diag.start_byte, diag.end_byte, sugg.clone()));
                    }
                }

                if diag.category == DiagnosticCategory::Spelling
                    && !diag.excerpt.is_empty()
                    && ui
                        .small_button(t!("bottom_panel.autocorrect_ignore_btn"))
                        .on_hover_text(t!("bottom_panel.autocorrect_ignore_tooltip"))
                        .clicked()
                {
                    output.ignore_word = Some(diag.excerpt.clone());
                }
            });
        });
    }
}
