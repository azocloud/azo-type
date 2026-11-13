//! Orchestration helpers wiring Settings/Bottom-Panel spell & grammar actions to the app state.

#[cfg(test)]
mod tests;

use std::sync::mpsc::Sender;

use egui::Context;

use crate::core::diff::CharSelection;
use crate::core::settings::{IdeSettings, SettingsTab};
use crate::core::spellcheck::UserDictionaryEntry;
use crate::language_check::LanguageCheckHost;
use crate::project::DiskProject;
use crate::ui::components::BottomPanelOutput;
use crate::ui::dialogs::SettingsModalOutput;

/// Safely replaces a UTF-8 byte range in the active buffer with `replacement`.
///
/// Returns `(updated_text, caret_char_offset_after_replacement)` if the range is valid.
pub fn apply_language_suggestion(
    project: &mut DiskProject,
    start_byte: usize,
    end_byte: usize,
    replacement: &str,
    auto_save: bool,
) -> Option<(String, usize)> {
    let active_path = project.active_file.clone();
    let buf = project.buffers.get_mut(&active_path)?;
    let text = buf.text.as_mut()?;
    if start_byte > end_byte
        || end_byte > text.len()
        || !text.is_char_boundary(start_byte)
        || !text.is_char_boundary(end_byte)
    {
        return None;
    }

    text.replace_range(start_byte..end_byte, replacement);
    let caret_byte = start_byte + replacement.len();
    let caret_char = text[..caret_byte].chars().count();
    let snapshot = text.clone();
    project.mark_active_modified(auto_save);
    Some((snapshot, caret_char))
}

/// Inserts or replaces a custom Tier-2 Hunspell dictionary entry by normalized language code.
pub fn register_user_dictionary(settings: &mut IdeSettings, entry: UserDictionaryEntry) -> String {
    let code = entry.lang_code.clone();
    let label = entry.label.clone();
    settings
        .user_dictionaries
        .retain(|d| !d.lang_code.eq_ignore_ascii_case(&code));
    settings.user_dictionaries.push(entry);
    settings
        .user_dictionaries
        .sort_by(|a, b| a.lang_code.cmp(&b.lang_code));
    format!("Loaded custom dictionary '{label}' for language '{code}'.")
}

/// Applies the result of a background `.dic`/`.aff` picker dialog to settings and re-checks.
pub fn apply_picked_dictionary_result(
    res: Result<UserDictionaryEntry, String>,
    settings: &mut IdeSettings,
    language_check: &mut LanguageCheckHost,
    project: &DiskProject,
    ui_lang: &str,
) -> String {
    match res {
        Ok(entry) => {
            let notice = register_user_dictionary(settings, entry);
            language_check.invalidate_dictionaries();
            if settings.spell_check_enabled {
                language_check.check_now(settings, project, ui_lang);
            }
            notice
        }
        Err(err) => err,
    }
}

/// Processes spell/grammar/dictionary actions emitted by the Settings dialog.
#[allow(clippy::too_many_arguments)]
pub fn handle_settings_language_actions<E: Send + 'static>(
    ctx: &Context,
    output: &SettingsModalOutput,
    settings: &mut IdeSettings,
    language_check: &mut LanguageCheckHost,
    project: &DiskProject,
    ui_lang: &str,
    io_tx: &Sender<E>,
    make_dict_event: fn(Result<UserDictionaryEntry, String>) -> E,
    status_notice: &mut String,
) {
    if let Some(ref lang) = output.download_dictionary_lang {
        language_check.download_pack(lang, ctx);
    }
    if let Some(ref lang) = output.remove_dictionary_lang {
        if let Err(err) = language_check.remove_pack(lang, settings, project, ui_lang) {
            *status_notice = err;
        }
    }
    if output.add_user_dictionary_requested {
        let tx = io_tx.clone();
        let repaint_ctx = ctx.clone();
        let start_dir = if project.has_disk_backing() {
            Some(project.root_dir.as_path())
        } else {
            None
        };
        crate::platform::dialogs::spawn_open_dictionary_dialog(
            start_dir,
            move |res| {
                let _ = tx.send(make_dict_event(res));
            },
            move || repaint_ctx.request_repaint(),
        );
    }
    if let Some(idx) = output.remove_user_dictionary_idx {
        if idx < settings.user_dictionaries.len() {
            settings.user_dictionaries.remove(idx);
            language_check.invalidate_dictionaries();
            if settings.spell_check_enabled {
                language_check.check_now(settings, project, ui_lang);
            }
        }
    }
    if output.clear_ignored_words {
        settings.ignored_words.clear();
        if settings.spell_check_enabled {
            language_check.check_now(settings, project, ui_lang);
        }
    }
    if output.check_language_now {
        language_check.check_now(settings, project, ui_lang);
    }
}

/// Processes Auto-Correction tab interactions and returns any edited text snapshot for collab/compile.
#[allow(clippy::too_many_arguments)]
pub fn handle_bottom_panel_language_actions(
    ctx: &Context,
    output: &BottomPanelOutput,
    settings: &mut IdeSettings,
    language_check: &mut LanguageCheckHost,
    project: &mut DiskProject,
    ui_lang: &str,
    pending_cursor_restore: &mut Option<CharSelection>,
    pending_scroll_to_char: &mut Option<usize>,
    show_settings_window: &mut bool,
    settings_tab: &mut SettingsTab,
) -> Option<String> {
    if output.open_spell_settings {
        *show_settings_window = true;
        *settings_tab = SettingsTab::SpellAndGrammar;
    }
    if let Some(ref lang) = output.download_dictionary_lang {
        language_check.download_pack(lang, ctx);
    }
    if let Some(ch) = output.jump_to_char {
        *pending_cursor_restore = Some(CharSelection::caret(ch));
        *pending_scroll_to_char = Some(ch);
    }
    if let Some(ref word) = output.ignore_word {
        if !settings.ignored_words.iter().any(|w| w == word) {
            settings.ignored_words.push(word.clone());
        }
        language_check.check_now(settings, project, ui_lang);
    }

    let mut edited_snapshot = None;
    if let Some((start_byte, end_byte, ref replacement)) = output.apply_suggestion {
        if let Some((snapshot, caret_char)) = apply_language_suggestion(
            project,
            start_byte,
            end_byte,
            replacement,
            settings.auto_save_on_edit,
        ) {
            *pending_cursor_restore = Some(CharSelection::caret(caret_char));
            *pending_scroll_to_char = Some(caret_char);
            language_check.check_now(settings, project, ui_lang);
            edited_snapshot = Some(snapshot);
        }
    } else if output.check_language_now {
        language_check.check_now(settings, project, ui_lang);
    }

    edited_snapshot
}
