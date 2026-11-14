//! Settings dialog tab for offline spellchecking, grammar checking, and dictionary management.

use egui::{Color32, ProgressBar, RichText, ScrollArea, Slider, Ui};
use rust_i18n::t;

use crate::core::settings::IdeSettings;
use crate::core::spellcheck::dictionary_catalog;
use crate::language_check::{DictionaryManager, DictionaryPackStatus};
use crate::ui::dialogs::settings::SettingsModalOutput;
use crate::ui::theme::ThemePalette;

/// Renders the Spell & Grammar tab inside the Settings sub-window.
pub fn render_spell_and_grammar_tab(
    ui: &mut Ui,
    settings: &mut IdeSettings,
    dict_manager: &DictionaryManager,
    palette: &ThemePalette,
    output: &mut SettingsModalOutput,
) {
    ScrollArea::vertical()
        .id_salt("settings_spell_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading(t!("settings.heading_spell_grammar"));
            ui.add_space(6.0);

            if ui
                .checkbox(
                    &mut settings.spell_check_enabled,
                    t!("settings.spell_enable_label"),
                )
                .changed()
            {
                output.check_language_now = true;
            }

            ui.add_enabled_ui(settings.spell_check_enabled, |ui| {
                ui.checkbox(
                    &mut settings.auto_spell_check,
                    t!("settings.spell_auto_check_label"),
                );
                if ui
                    .checkbox(
                        &mut settings.grammar_check_enabled,
                        t!("settings.spell_grammar_enable_label"),
                    )
                    .changed()
                {
                    output.check_language_now = true;
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(t!("settings.spell_debounce_label"));
                let mut debounce = settings.spell_check_debounce_ms as u32;
                if ui
                    .add(Slider::new(&mut debounce, 150..=2000).suffix(" ms"))
                    .changed()
                {
                    settings.spell_check_debounce_ms = debounce as u64;
                }
            });

            ui.add_space(4.0);
            let catalog = dictionary_catalog();
            ui.horizontal(|ui| {
                ui.label(t!("settings.spell_language_label"));
                let selected_label = if settings.spell_check_language == "auto" {
                    t!("settings.spell_lang_auto").to_string()
                } else if let Some(pack) = catalog.dictionaries.get(&settings.spell_check_language)
                {
                    format!("{} ({})", pack.display_name, settings.spell_check_language)
                } else {
                    settings.spell_check_language.clone()
                };

                egui::ComboBox::from_id_salt("settings_spell_lang_combo")
                    .selected_text(selected_label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(
                                &mut settings.spell_check_language,
                                "auto".to_string(),
                                t!("settings.spell_lang_auto"),
                            )
                            .changed()
                        {
                            output.check_language_now = true;
                        }
                        for (code, pack) in &catalog.dictionaries {
                            let label = format!("{} ({})", pack.display_name, code);
                            if ui
                                .selectable_value(
                                    &mut settings.spell_check_language,
                                    code.clone(),
                                    label,
                                )
                                .changed()
                            {
                                output.check_language_now = true;
                            }
                        }
                        for entry in &settings.user_dictionaries {
                            let label = format!("{} ({})", entry.label, entry.lang_code);
                            if ui
                                .selectable_value(
                                    &mut settings.spell_check_language,
                                    entry.lang_code.clone(),
                                    label,
                                )
                                .changed()
                            {
                                output.check_language_now = true;
                            }
                        }
                    });

                if settings.spell_check_enabled
                    && ui.button(t!("settings.spell_check_now_btn")).clicked()
                {
                    output.check_language_now = true;
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            ui.label(RichText::new(t!("settings.spell_tier1_heading")).strong());
            ui.label(
                RichText::new(t!("settings.spell_tier1_desc"))
                    .small()
                    .color(palette.text_muted),
            );
            ui.add_space(6.0);

            for (code, pack) in &catalog.dictionaries {
                let status = dict_manager.pack_status(code);
                let size_mb = pack.total_size_bytes() as f64 / (1024.0 * 1024.0);
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let grammar_tag = if pack.has_grammar_model {
                            " + Grammar"
                        } else {
                            " + Typography"
                        };
                        ui.label(
                            RichText::new(format!("{} ({})", pack.display_name, pack.locale_tag))
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("{:.1} MB{}", size_mb, grammar_tag))
                                .small()
                                .color(palette.text_muted),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            match status {
                                DictionaryPackStatus::NotInstalled => {
                                    if ui.button(t!("settings.typst_download_btn")).clicked() {
                                        output.download_dictionary_lang = Some(code.clone());
                                    }
                                    ui.label(
                                        RichText::new(t!("settings.spell_tier1_not_installed"))
                                            .small()
                                            .color(palette.text_muted),
                                    );
                                }
                                DictionaryPackStatus::Downloading => {
                                    ui.add_enabled(
                                        false,
                                        egui::Button::new(t!("settings.typst_downloading_btn")),
                                    );
                                    ui.label(
                                        RichText::new(t!("settings.spell_tier1_downloading"))
                                            .small()
                                            .color(palette.syn_operator),
                                    );
                                }
                                DictionaryPackStatus::Installed => {
                                    if ui.button(t!("settings.typst_remove_btn")).clicked() {
                                        output.remove_dictionary_lang = Some(code.clone());
                                    }
                                    ui.label(
                                        RichText::new(t!("settings.spell_tier1_installed"))
                                            .small()
                                            .color(palette.syn_string),
                                    );
                                }
                            }
                        });
                    });

                    if let Some(progress) = dict_manager.download_progress(code) {
                        ui.add_space(4.0);
                        ui.add(ProgressBar::new(progress.fraction()).show_percentage());
                        let dl_mb = progress.downloaded_bytes as f64 / (1024.0 * 1024.0);
                        let tot_mb = (progress.total_bytes as f64 / (1024.0 * 1024.0)).max(0.01);
                        ui.label(
                            RichText::new(t!(
                                "settings.spell_download_progress",
                                lang = code,
                                file = progress.current_filename,
                                idx = progress.current_file_index,
                                total = progress.total_files,
                                downloaded = format!("{dl_mb:.1}"),
                                size = format!("{tot_mb:.1}")
                            ))
                            .small()
                            .color(palette.text_muted),
                        );
                    }
                });
            }

            if let Some(err) = dict_manager.last_error() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(err)
                        .small()
                        .color(Color32::from_rgb(220, 60, 60)),
                );
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("settings.spell_tier2_heading")).strong());
                if ui.button(t!("settings.spell_add_dict_btn")).clicked() {
                    output.add_user_dictionary_requested = true;
                }
            });
            ui.label(
                RichText::new(t!("settings.spell_tier2_desc"))
                    .small()
                    .color(palette.text_muted),
            );
            ui.add_space(4.0);

            if settings.user_dictionaries.is_empty() {
                ui.label(
                    RichText::new(t!("settings.spell_no_custom_dicts"))
                        .small()
                        .color(palette.text_muted),
                );
            } else {
                for (idx, entry) in settings.user_dictionaries.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("[{}]", entry.lang_code)).strong());
                        ui.label(
                            RichText::new(format!(
                                "{} ({})",
                                entry.label,
                                entry
                                    .dic_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("dictionary.dic")
                            ))
                            .small(),
                        );
                        if ui.button(t!("settings.typst_remove_btn")).clicked() {
                            output.remove_user_dictionary_idx = Some(idx);
                        }
                    });
                }
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(t!(
                    "settings.spell_ignored_count",
                    count = settings.ignored_words.len()
                ));
                if !settings.ignored_words.is_empty()
                    && ui.button(t!("settings.spell_clear_ignored_btn")).clicked()
                {
                    output.clear_ignored_words = true;
                }
            });
        });
}

#[cfg(test)]
#[path = "../../tests/ui/dialogs/settings_spell_tests.rs"]
mod tests;
