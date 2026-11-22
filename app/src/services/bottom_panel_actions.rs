//! Orchestration helpers wiring Bottom Panel search, replace, and output actions to app state.

#[cfg(test)]
#[path = "../tests/services/bottom_panel_actions_tests.rs"]
mod tests;

use egui::Context;

use crate::core::diff::CharSelection;
use crate::core::file_kind::FileKind;
use crate::core::settings::{IdeSettings, SettingsTab};
use crate::engine::DiagnosticItem;
use crate::language_check::LanguageCheckHost;
use crate::platform::menu::MenuAction;
use crate::project::DiskProject;
use crate::services::language_actions::handle_bottom_panel_language_actions;
use crate::ui::components::{
    render_bottom_panel, BottomPanelOutput, BottomPanelParams, BottomPanelState,
};

/// Returns `true` when `action` is executed inline by `editor_actions::apply_editor_context_action`
/// and must not be re-dispatched into `pending_editor_context_action` on the next frame.
pub fn is_inline_editor_menu_action(action: MenuAction) -> bool {
    matches!(
        action,
        MenuAction::ToggleLineComment
            | MenuAction::ToggleBlockComment
            | MenuAction::IndentLines
            | MenuAction::OutdentLines
            | MenuAction::DuplicateLine
            | MenuAction::MoveLineUp
            | MenuAction::MoveLineDown
    )
}

/// Appends `snippet` to the active project buffer and returns the updated text snapshot.
pub fn append_snippet_to_active_buffer(project: &mut DiskProject, snippet: &str) -> Option<String> {
    project.active_buffer_mut().and_then(|buf| {
        buf.text.as_mut().map(|text| {
            text.push_str(snippet);
            text.clone()
        })
    })
}

/// Applies Search & Replace mutations and match selection jumps emitted by the bottom panel.
///
/// Returns `Some(updated_text)` when the active buffer text was replaced.
pub fn handle_bottom_panel_search_actions(
    ctx: &Context,
    output: &BottomPanelOutput,
    project: &mut DiskProject,
    inspect_asset_mode: &mut bool,
    pending_cursor_restore: &mut Option<CharSelection>,
    pending_scroll_to_char: &mut Option<usize>,
) -> Option<String> {
    let mut replaced = None;
    if let Some(ref new_text) = output.replace_in_active_text {
        if let Some(buf) = project.active_buffer_mut() {
            buf.text = Some(new_text.clone());
            replaced = Some(new_text.clone());
        }
        ctx.request_repaint();
    }

    if let Some((start_c, end_c)) = output.jump_to_selection {
        let max_chars = project
            .active_buffer()
            .and_then(|b| b.text.as_deref())
            .map(|t| t.chars().count())
            .unwrap_or(0);
        let start_clamped = start_c.min(max_chars);
        let end_clamped = end_c.min(max_chars);
        *pending_cursor_restore = Some(CharSelection {
            primary_char: end_clamped,
            secondary_char: start_clamped,
        });
        *pending_scroll_to_char = Some(start_clamped);
        *inspect_asset_mode = false;
        ctx.request_repaint();
    }

    replaced
}

/// Applies all actions emitted by [`crate::ui::components::render_bottom_panel`].
///
/// Returns `(open_diagnostic, edited_text_snapshot)` so `AzoTypeApp` can jump to diagnostics
/// and trigger collab/compile sync when buffer text changed.
#[allow(clippy::too_many_arguments)]
pub fn apply_bottom_panel_output(
    ctx: &Context,
    output: &BottomPanelOutput,
    bottom_panel_visible: &mut bool,
    settings: &mut IdeSettings,
    language_check: &mut LanguageCheckHost,
    project: &mut DiskProject,
    ui_lang: &str,
    inspect_asset_mode: &mut bool,
    pending_cursor_restore: &mut Option<CharSelection>,
    pending_scroll_to_char: &mut Option<usize>,
    show_settings_window: &mut bool,
    settings_tab: &mut SettingsTab,
) -> (Option<DiagnosticItem>, Option<String>) {
    if output.close_requested {
        *bottom_panel_visible = false;
    }

    let search_edited = handle_bottom_panel_search_actions(
        ctx,
        output,
        project,
        inspect_asset_mode,
        pending_cursor_restore,
        pending_scroll_to_char,
    );

    let lang_edited = handle_bottom_panel_language_actions(
        ctx,
        output,
        settings,
        language_check,
        project,
        ui_lang,
        pending_cursor_restore,
        pending_scroll_to_char,
        show_settings_window,
        settings_tab,
    );

    (
        output.open_diagnostic.clone(),
        search_edited.or(lang_edited),
    )
}

/// Renders the collapsible bottom panel and applies all emitted search, diagnostic, and language actions.
#[allow(clippy::too_many_arguments)]
pub fn render_and_handle_bottom_panel(
    ctx: &Context,
    ui: &mut egui::Ui,
    state: &mut BottomPanelState,
    diagnostics: &[DiagnosticItem],
    settings: &mut IdeSettings,
    language_check: &mut LanguageCheckHost,
    project: &mut DiskProject,
    ui_lang: &str,
    last_editor_selection: Option<CharSelection>,
    inspect_asset_mode: &mut bool,
    pending_cursor_restore: &mut Option<CharSelection>,
    pending_scroll_to_char: &mut Option<usize>,
    show_settings_window: &mut bool,
    settings_tab: &mut SettingsTab,
) -> (Option<DiagnosticItem>, Option<String>) {
    let active_text = project
        .active_buffer()
        .and_then(|b| b.text.as_deref())
        .unwrap_or("");
    let active_cursor_char = last_editor_selection.map_or(0, |s| s.primary_char);
    let is_active_typst = FileKind::from_path(&project.active_file) == FileKind::Typst;
    let panel_out = render_bottom_panel(
        ui,
        BottomPanelParams {
            dark_mode: settings.dark_mode,
            state,
            diagnostics,
            active_file: &project.active_file,
            is_active_typst,
            language: ui_lang,
            spell_check_enabled: &mut settings.spell_check_enabled,
            auto_spell_check: &mut settings.auto_spell_check,
            language_check,
            active_text,
            active_cursor_char,
        },
    );
    apply_bottom_panel_output(
        ctx,
        &panel_out,
        &mut state.visible,
        settings,
        language_check,
        project,
        ui_lang,
        inspect_asset_mode,
        pending_cursor_restore,
        pending_scroll_to_char,
        show_settings_window,
        settings_tab,
    )
}
